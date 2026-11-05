//! Validated server-side row paging endpoint. Never accepts raw SQL.
use crate::{
    db::types::{DbError, DbErrorKind, RelationId, RowOptions, SortDirection, SortSpec},
    error::{AppError, ErrorKind},
    state::AppState,
};
use askama::Template;
use axum::{
    extract::{RawQuery, State},
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
};
#[derive(Clone)]
struct Header {
    name: String,
    native_type: String,
    asc: String,
    desc: String,
    sort_direction: Option<&'static str>,
    is_ascending: bool,
    is_descending: bool,
    sortable: bool,
    numeric: bool,
    pk: bool,
    nullable: bool,
    /// Must be supplied when inserting (no default, not nullable).
    required: bool,
    /// Simple scalar types the row editor can safely write as text.
    editable: bool,
}
#[derive(Clone)]
struct Value {
    text: String,
    native_type: String,
    is_null: bool,
    truncated: bool,
    detail: String,
    class: &'static str,
    /// Set for single-column foreign keys whose target key is a primary key.
    fk_href: String,
}
#[derive(Clone)]
struct DisplayRow {
    number: u64,
    key_json: String,
    values: Vec<Value>,
}
#[derive(Template)]
#[template(path = "components/data_page.html")]
struct PageTemplate {
    active: &'static str,
    links: super::catalog::RelationLinks,
    /// Selected profile id, or empty for the default profile.
    profile: String,
    filter: String,
    page_size: u32,
    has_spatial: bool,
    geojson_active: bool,
    geojson_url: String,
    sort: Option<String>,
    direction: Option<&'static str>,
    selected_columns: Vec<String>,
    available_columns: Vec<ColumnView>,
    schema_name: String,
    relation_name: String,
    headers: Vec<Header>,
    rows: Vec<DisplayRow>,
    previous_url: String,
    next_url: String,
    page_number: u32,
    sizes: Vec<SizeOption>,
    export_view_csv: String,
    export_all_csv: String,
    export_all_json: String,
    export_zip_1: String,
    export_zip_2: String,
    stable: bool,
    /// The caller may insert, update and delete rows here (grant, writable profile, key).
    can_edit: bool,
    truncated_columns: bool,
    shown_from: u64,
    shown_to: u64,
    approximate_total: String,
    /// "80.5 MB" from catalog statistics, empty when unavailable.
    size_label: String,
}
struct SizeOption {
    value: u32,
    href: String,
    selected: bool,
}
#[derive(Clone)]
struct ColumnView {
    name: String,
    native_type: String,
    selected: bool,
}
fn invalid() -> AppError {
    AppError::new(ErrorKind::InvalidInput)
}
pub(super) fn classified(db: DbError) -> AppError {
    AppError::new(match db.kind {
        DbErrorKind::InvalidIdentifier => ErrorKind::InvalidInput,
        DbErrorKind::PermissionDenied => ErrorKind::Forbidden,
        DbErrorKind::TimedOut => ErrorKind::TimedOut,
        DbErrorKind::ConnectionUnavailable | DbErrorKind::AuthenticationFailed => {
            ErrorKind::DatabaseUnavailable
        }
        DbErrorKind::Unsupported => ErrorKind::InvalidInput,
        DbErrorKind::Rejected => ErrorKind::Rejected,
        DbErrorKind::Conflict => ErrorKind::Conflict,
        DbErrorKind::Cancelled => ErrorKind::Cancelled,
        DbErrorKind::Internal => ErrorKind::Internal,
    })
}
pub(super) fn name(value: &str) -> Result<String, AppError> {
    if value.is_empty() || value.len() > 63 || value.chars().any(char::is_control) {
        Err(invalid())
    } else {
        Ok(value.to_string())
    }
}
/// Database text is untrusted: make control characters and bidirectional
/// overrides visible instead of letting them reorder or hide surrounding text.
/// Newline and tab are kept for readable multi-line values.
pub(super) fn visible_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        let hidden = (c.is_control() && c != '\n' && c != '\t')
            || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}');
        if hidden {
            out.push_str(&format!("\\u{{{:04X}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}
fn value_class(native_type: &str) -> &'static str {
    let native = native_type.to_ascii_lowercase();
    if native.contains("json") {
        "json-value"
    } else if native.ends_with("[]") {
        "array-value"
    } else if native == "boolean" || native == "bool" {
        "bool-value"
    } else if matches!(native.as_str(), "uuid" | "inet" | "cidr" | "macaddr") {
        "mono-value"
    } else if native.starts_with("timestamp") || matches!(native.as_str(), "date" | "time") {
        "ts-value"
    } else if native == "bytea" || native.contains("blob") || native.contains("binary") {
        "binary-value"
    } else if matches!(
        native.as_str(),
        "smallint" | "integer" | "bigint" | "real" | "double precision"
    ) || native.starts_with("numeric")
        || native.starts_with("decimal")
    {
        "numeric"
    } else {
        ""
    }
}
pub(super) fn detail_value(cell: &crate::db::types::Cell, native_type: &str) -> String {
    use crate::db::types::Cell;
    match cell {
        Cell::Text(text) | Cell::Decimal(text) => {
            if native_type.to_ascii_lowercase().contains("json") {
                serde_json::from_str::<serde_json::Value>(text)
                    .ok()
                    .and_then(|value| serde_json::to_string_pretty(&value).ok())
                    .filter(|pretty| pretty.len() <= 1024)
                    .unwrap_or_else(|| text.clone())
            } else {
                text.clone()
            }
        }
        Cell::Integer(number) => number.to_string(),
        Cell::Boolean(value) => value.to_string(),
        Cell::Bytes(bytes) => format!(
            "\\x{}",
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
        Cell::Truncated { preview, .. } => preview.clone(),
        Cell::Unsupported { type_name } => format!("Unsupported {type_name}"),
        Cell::Null => "NULL".into(),
    }
}
#[derive(Clone)]
pub(super) struct Navigation {
    pub(super) id: RelationId,
    pub(super) options: RowOptions,
    pub(super) profile: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataQuery {
    pub(super) schema: String,
    pub(super) relation: String,
    pub(super) size: Option<u32>,
    pub(super) page: Option<u32>,
    pub(super) sort: Option<String>,
    pub(super) direction: Option<String>,
    pub(super) cursor: Option<String>,
    pub(super) filter: Option<String>,
    pub(super) spatial: Option<String>,
    pub(super) pick: Option<String>,
    pub(super) profile: Option<String>,
    #[serde(default)]
    pub(super) column: Vec<String>,
}

/// Split a raw query into single-valued pairs and repeated `column` values.
/// serde's urlencoded decoder cannot read repeated keys into a `Vec`, yet the grid's
/// own links and column picker repeat `column=`. Unknown keys and duplicated
/// single-valued keys are rejected.
pub(super) fn split_query(
    raw: Option<&str>,
    allowed: &[&str],
) -> Result<(std::collections::BTreeMap<String, String>, Vec<String>), AppError> {
    let raw = raw.unwrap_or_default();
    if raw.len() > 4096 {
        return Err(invalid());
    }
    let mut single = std::collections::BTreeMap::new();
    let mut columns = vec![];
    for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
        if key == "column" && allowed.contains(&"column") {
            columns.push(value.into_owned());
        } else if allowed.contains(&key.as_ref())
            && single
                .insert(key.into_owned(), value.into_owned())
                .is_none()
        {
        } else {
            return Err(invalid());
        }
    }
    Ok((single, columns))
}

impl DataQuery {
    pub(super) fn from_raw(raw: Option<&str>) -> Result<Self, AppError> {
        let (mut map, column) = split_query(
            raw,
            &[
                "schema",
                "relation",
                "size",
                "page",
                "sort",
                "direction",
                "cursor",
                "filter",
                "spatial",
                "pick",
                "column",
                "profile",
            ],
        )?;
        let number = |value: Option<String>| {
            value
                .map(|v| v.parse::<u32>().map_err(|_| invalid()))
                .transpose()
        };
        Ok(Self {
            schema: map.remove("schema").ok_or_else(invalid)?,
            relation: map.remove("relation").ok_or_else(invalid)?,
            size: number(map.remove("size"))?,
            page: number(map.remove("page"))?,
            sort: map.remove("sort"),
            direction: map.remove("direction"),
            cursor: map.remove("cursor"),
            filter: map.remove("filter"),
            spatial: map.remove("spatial"),
            pick: map.remove("pick"),
            profile: map.remove("profile"),
            column,
        })
    }
}

pub(super) fn parse(query: DataQuery) -> Result<Navigation, AppError> {
    if query.column.len() > 24
        || query
            .filter
            .as_ref()
            .is_some_and(|filter| filter.len() > 128)
    {
        return Err(invalid());
    }
    let spatial_geojson = match query.spatial.as_deref() {
        None => false,
        Some("geojson") => true,
        Some(_) => return Err(invalid()),
    };
    let column_selection_explicit = match query.pick.as_deref() {
        None => false,
        Some("1") => true,
        Some(_) => return Err(invalid()),
    };
    if query.sort.is_some() != query.direction.is_some() {
        return Err(invalid());
    }
    let sort = query.sort.map(|column| name(&column)).transpose()?;
    let direction = query
        .direction
        .map(|direction| match direction.as_str() {
            "asc" => Ok(SortDirection::Asc),
            "desc" => Ok(SortDirection::Desc),
            _ => Err(invalid()),
        })
        .transpose()?;
    let cursor = query
        .cursor
        .map(|cursor| {
            if cursor.len() == 64 && cursor.bytes().all(|b| b.is_ascii_hexdigit()) {
                Ok(cursor)
            } else {
                Err(invalid())
            }
        })
        .transpose()?;
    let options = RowOptions {
        limit: query.size.unwrap_or(100),
        page: query.page.unwrap_or(0),
        selected_columns: query
            .column
            .iter()
            .map(|c| name(c))
            .collect::<Result<_, _>>()?,
        column_selection_explicit,
        sort: sort
            .zip(direction)
            .map(|(column, direction)| SortSpec { column, direction }),
        cursor,
        filter: query.filter.filter(|filter| !filter.is_empty()),
        spatial_geojson,
    };
    if options.limit == 0
        || options.limit > 1000
        || (options.page > 100 && options.cursor.is_none())
        || (options.cursor.is_none() && u64::from(options.page) * u64::from(options.limit) > 10_000)
    {
        return Err(invalid());
    }
    Ok(Navigation {
        profile: crate::state::checked_profile(query.profile)?,
        id: RelationId {
            namespace: name(&query.schema)?,
            name: name(&query.relation)?,
        },
        options,
    })
}
fn link(nav: &Navigation, page: u32, cursor: Option<&str>, sort: Option<SortSpec>) -> String {
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    q.append_pair("schema", &nav.id.namespace);
    q.append_pair("relation", &nav.id.name);
    q.append_pair("size", &nav.options.limit.to_string());
    if let Some(profile) = nav.profile.as_deref() {
        q.append_pair("profile", profile);
    }
    if let Some(sort) = sort {
        q.append_pair("sort", &sort.column);
        q.append_pair(
            "direction",
            match sort.direction {
                SortDirection::Asc => "asc",
                SortDirection::Desc => "desc",
            },
        );
    }
    for col in &nav.options.selected_columns {
        q.append_pair("column", col);
    }
    if nav.options.column_selection_explicit {
        q.append_pair("pick", "1");
    }
    if let Some(filter) = nav.options.filter.as_deref() {
        q.append_pair("filter", filter);
    }
    if nav.options.spatial_geojson {
        q.append_pair("spatial", "geojson");
    }
    if let Some(token) = cursor {
        q.append_pair("cursor", token);
    } else if page > 0 {
        q.append_pair("page", &page.to_string());
    }
    format!("/catalog/data?{}", q.finish())
}
/// Bounded server-side export links; the export route re-validates every parameter.
fn export_link(nav: &Navigation, format: &str, scope: &str, page: u32) -> String {
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    q.append_pair("schema", &nav.id.namespace);
    q.append_pair("relation", &nav.id.name);
    q.append_pair("format", format);
    q.append_pair("scope", scope);
    q.append_pair("size", &nav.options.limit.to_string());
    if scope == "page" && page > 0 {
        q.append_pair("page", &page.to_string());
    }
    if let Some(profile) = nav.profile.as_deref() {
        q.append_pair("profile", profile);
    }
    if scope == "page" {
        if let Some(sort) = nav.options.sort.as_ref() {
            q.append_pair("sort", &sort.column);
            q.append_pair(
                "direction",
                match sort.direction {
                    SortDirection::Asc => "asc",
                    SortDirection::Desc => "desc",
                },
            );
        }
        if let Some(filter) = nav.options.filter.as_deref() {
            q.append_pair("filter", filter);
        }
        for col in &nav.options.selected_columns {
            q.append_pair("column", col);
        }
    }
    format!("/catalog/export?{}", q.finish())
}
/// Related-table ZIP: this relation plus the relations its foreign keys reach.
fn zip_link(nav: &Navigation, depth: u8) -> String {
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    q.append_pair("schema", &nav.id.namespace);
    q.append_pair("relation", &nav.id.name);
    q.append_pair("format", "zip");
    q.append_pair("scope", "related");
    q.append_pair("depth", &depth.to_string());
    if let Some(profile) = nav.profile.as_deref() {
        q.append_pair("profile", profile);
    }
    format!("/catalog/export?{}", q.finish())
}
pub async fn data(
    State(state): State<AppState>,
    auth: crate::web::auth::Auth,
    RawQuery(raw): RawQuery,
) -> Result<Response, AppError> {
    let query = DataQuery::from_raw(raw.as_deref())?;
    let nav = parse(query)?;
    let pid = state.effective_profile(nav.profile.as_deref());
    auth.authorize(
        &state,
        crate::auth::Action::View,
        &crate::auth::Resource::relation(&pid, &nav.id.namespace, &nav.id.name),
    )
    .await?;
    let catalog = state.adapter(nav.profile.as_deref())?;
    let page = catalog
        .fetch_rows(&nav.id, nav.options.clone())
        .await
        .map_err(classified)?;
    let headers =
        page.columns
            .iter()
            .map(|c| Header {
                name: c.name.clone(),
                native_type: c.native_type.clone(),
                sortable: catalog.sort_supported(&c.native_type),
                numeric: value_class(&c.native_type) == "numeric",
                pk: c.primary_key,
                nullable: c.nullable,
                required: !c.nullable && c.default_expression.is_none(),
                editable: !matches!(
                    value_class(&c.native_type),
                    "json-value" | "array-value" | "binary-value"
                ) && {
                    let native = c.native_type.to_ascii_lowercase();
                    !crate::db::postgres::row_query::is_spatial_type(&native)
                        && !native.contains("tsvector")
                        && !native.contains("xml")
                },
                asc: {
                    let clear = nav.options.sort.as_ref().is_some_and(|sort| {
                        sort.column == c.name && sort.direction == SortDirection::Asc
                    });
                    if clear {
                        link(&nav, 0, None, None)
                    } else {
                        link(
                            &nav,
                            0,
                            None,
                            Some(SortSpec {
                                column: c.name.clone(),
                                direction: SortDirection::Asc,
                            }),
                        )
                    }
                },
                desc: {
                    let clear = nav.options.sort.as_ref().is_some_and(|sort| {
                        sort.column == c.name && sort.direction == SortDirection::Desc
                    });
                    if clear {
                        link(&nav, 0, None, None)
                    } else {
                        link(
                            &nav,
                            0,
                            None,
                            Some(SortSpec {
                                column: c.name.clone(),
                                direction: SortDirection::Desc,
                            }),
                        )
                    }
                },
                sort_direction: nav.options.sort.as_ref().and_then(|sort| {
                    (sort.column == c.name).then_some(match sort.direction {
                        SortDirection::Asc => "ascending",
                        SortDirection::Desc => "descending",
                    })
                }),
                is_ascending: nav.options.sort.as_ref().is_some_and(|sort| {
                    sort.column == c.name && sort.direction == SortDirection::Asc
                }),
                is_descending: nav.options.sort.as_ref().is_some_and(|sort| {
                    sort.column == c.name && sort.direction == SortDirection::Desc
                }),
            })
            .collect::<Vec<_>>();
    // Single-column foreign keys turn their cells into links (best effort, bounded).
    let fk_targets: std::collections::HashMap<String, (RelationId, String)> = if matches!(
        catalog.engine(),
        crate::db::types::Engine::PostgreSql | crate::db::types::Engine::Sqlite
    ) {
        catalog
            .list_constraints(&nav.id)
            .await
            .map(|list| {
                list.items
                    .into_iter()
                    .filter(|k| {
                        k.kind == crate::db::types::ConstraintKind::ForeignKey
                            && k.columns.len() == 1
                    })
                    .filter_map(|k| {
                        Some((
                            k.columns.into_iter().next()?,
                            (k.references?, k.referenced_columns.into_iter().next()?),
                        ))
                    })
                    .filter(|(_, (target, _target_column))| {
                        auth.0.allows(
                            crate::auth::Action::View,
                            &crate::auth::Resource::relation(&pid, &target.namespace, &target.name),
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    // Enum columns get the pill treatment (best effort, one bounded metadata read).
    let enum_columns: std::collections::HashSet<String> = catalog
        .column_details(&nav.id)
        .await
        .map(|list| {
            list.items
                .into_iter()
                .filter(|d| matches!(d.enum_values, crate::db::types::Availability::Available(_)))
                .map(|d| d.column.name)
                .collect()
        })
        .unwrap_or_default();
    let mut fk_targets = fk_targets;
    let target_checks = fk_targets.values().cloned().collect::<Vec<_>>();
    for (target, target_column) in target_checks {
        let is_unique = catalog
            .is_unique_key(&target, std::slice::from_ref(&target_column))
            .await
            .unwrap_or(false);
        if !is_unique {
            fk_targets.retain(|_, (candidate, candidate_column)| {
                candidate != &target || candidate_column != &target_column
            });
        }
    }
    let has_spatial = page.columns.iter().any(|column| {
        let native = column.native_type.to_ascii_lowercase();
        crate::db::postgres::row_query::is_spatial_type(&native)
    });
    let shown_from = if page.rows.is_empty() {
        0
    } else {
        u64::from(page.page) * u64::from(nav.options.limit) + 1
    };
    let shown_to = if page.rows.is_empty() {
        0
    } else {
        shown_from + page.rows.len() as u64 - 1
    };
    let size_label = catalog
        .relation_statistics(&nav.id)
        .await
        .ok()
        .and_then(|stats| match stats.total_bytes {
            crate::db::types::Availability::Available(bytes) => {
                Some(super::catalog::human_bytes(bytes))
            }
            crate::db::types::Availability::Unavailable => None,
        })
        .unwrap_or_default();
    let approximate_total = match (&page.approximate_total, nav.options.filter.as_ref()) {
        (_, Some(_)) | (crate::db::types::Availability::Unavailable, None) => {
            "unavailable".to_string()
        }
        (crate::db::types::Availability::Available(total), None) => total.to_string(),
    };
    let previous_url = if page.has_previous {
        link(
            &nav,
            page.page.saturating_sub(1),
            page.previous_cursor.as_deref(),
            nav.options.sort.clone(),
        )
    } else {
        String::new()
    };
    let next_url = if page.has_more && (page.page < 100 || page.next_cursor.is_some()) {
        link(
            &nav,
            page.page.saturating_add(1),
            page.next_cursor.as_deref(),
            nav.options.sort.clone(),
        )
    } else {
        String::new()
    };
    let mut rendered = Vec::new();
    for (index, (cells, previews)) in page.rows.into_iter().zip(page.previews).enumerate() {
        let key_pairs = page
            .columns
            .iter()
            .zip(&cells)
            .filter(|(column, _)| column.primary_key)
            .map(|(column, cell)| (column.name.clone(), detail_value(cell, &column.native_type)))
            .collect::<Vec<_>>();
        let key_json = serde_json::to_string(&key_pairs).unwrap_or_else(|_| "[]".into());
        let values = cells
            .iter()
            .zip(previews)
            .zip(page.columns.iter())
            .map(|((cell, preview), column)| {
                let mut detail = detail_value(cell, &preview.type_name);
                if let Some(spatial) = preview.detail.as_deref() {
                    detail.push_str("\n\nGeoJSON detail:\n");
                    detail.push_str(spatial);
                }
                let fk_href = match (fk_targets.get(&column.name), cell) {
                    (Some((target, target_column)), c)
                        if !matches!(c, crate::db::types::Cell::Null) && !preview.truncated =>
                    {
                        let payload = serde_json::json!({
                            "schema": target.namespace,
                            "relation": target.name,
                            "key": target_column,
                            "value": preview.text,
                            "profile": nav.profile,
                        });
                        format!(
                            "#fk={}",
                            url::form_urlencoded::byte_serialize(payload.to_string().as_bytes())
                                .collect::<String>()
                        )
                    }
                    _ => String::new(),
                };
                Value {
                    fk_href,
                    text: visible_text(&preview.text),
                    class: if enum_columns.contains(&column.name) {
                        "enum-value"
                    } else {
                        value_class(&preview.type_name)
                    },
                    native_type: preview.type_name,
                    truncated: preview.truncated,
                    is_null: matches!(cell, crate::db::types::Cell::Null),
                    detail: visible_text(&detail),
                }
            })
            .collect();
        rendered.push(DisplayRow {
            number: shown_from + index as u64,
            key_json,
            values,
        });
    }
    let sizes: Vec<SizeOption> = [25u32, 50, 100, 250, 500, 1000]
        .into_iter()
        .map(|value| {
            let mut other = nav.clone();
            other.options.limit = value;
            SizeOption {
                value,
                href: link(&other, 0, None, other.options.sort.clone()),
                selected: value == nav.options.limit,
            }
        })
        .collect();
    let export_view_csv = export_link(&nav, "csv", "page", page.page);
    let export_all_csv = export_link(&nav, "csv", "relation", 0);
    let export_all_json = export_link(&nav, "json", "relation", 0);
    let export_zip_1 = zip_link(&nav, 1);
    let export_zip_2 = zip_link(&nav, 2);
    let mut geojson_nav = nav.clone();
    geojson_nav.options.spatial_geojson = !nav.options.spatial_geojson;
    let geojson_url = link(&geojson_nav, 0, None, geojson_nav.options.sort.clone());
    let can_edit = !auth.0.is_open()
        && state.sql_settings().enabled
        && state
            .workspace_for(nav.profile.as_deref())
            .is_some_and(|w| w.writable())
        && auth.0.allows(
            crate::auth::Action::Edit,
            &crate::auth::Resource::relation(&pid, &nav.id.namespace, &nav.id.name),
        )
        && page.columns.iter().any(|c| c.primary_key);
    let view = PageTemplate {
        active: "data",
        links: super::catalog::relation_links(&nav.id, nav.profile.as_deref()),
        can_edit,
        profile: nav.profile.clone().unwrap_or_default(),
        filter: nav.options.filter.clone().unwrap_or_default(),
        page_size: nav.options.limit,
        has_spatial,
        geojson_active: nav.options.spatial_geojson,
        geojson_url,
        sort: nav.options.sort.as_ref().map(|sort| sort.column.clone()),
        direction: nav.options.sort.as_ref().map(|sort| match sort.direction {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }),
        selected_columns: nav.options.selected_columns.clone(),
        available_columns: page
            .available_columns
            .iter()
            .map(|column| ColumnView {
                name: column.name.clone(),
                native_type: column.native_type.clone(),
                selected: if nav.options.column_selection_explicit {
                    nav.options.selected_columns.contains(&column.name)
                } else {
                    nav.options.selected_columns.is_empty()
                        || nav.options.selected_columns.contains(&column.name)
                },
            })
            .collect(),
        schema_name: nav.id.namespace.clone(),
        relation_name: nav.id.name.clone(),
        export_view_csv,
        export_all_csv,
        export_all_json,
        export_zip_1,
        export_zip_2,
        headers,
        rows: rendered,
        previous_url,
        next_url,
        page_number: page.page.saturating_add(1),
        sizes,
        stable: page.stable_order,
        truncated_columns: page.truncated_columns,
        shown_from,
        shown_to,
        approximate_total,
        size_label,
    };
    let body = view.render().map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForeignRowQuery {
    schema: String,
    relation: String,
    key: String,
    value: String,
    profile: Option<String>,
}

/// Fetch the referenced row's bounded cell values for a foreign-key sheet.
pub async fn foreign_row(
    State(state): State<AppState>,
    auth: super::auth::Auth,
    axum::extract::Query(query): axum::extract::Query<ForeignRowQuery>,
) -> Result<Response, AppError> {
    let schema = name(&query.schema)?;
    let relation = name(&query.relation)?;
    let key_name = name(&query.key)?;
    if query.value.len() > 65_536 || query.value.chars().any(char::is_control) {
        return Err(invalid());
    }
    let profile = crate::state::checked_profile(query.profile)?;
    let pid = state.effective_profile(profile.as_deref());
    auth.authorize(
        &state,
        crate::auth::Action::View,
        &crate::auth::Resource::relation(&pid, &schema, &relation),
    )
    .await?;
    let catalog = state.adapter(profile.as_deref())?;
    let id = RelationId {
        namespace: schema,
        name: relation,
    };
    let columns = catalog
        .describe_columns(&id)
        .await
        .map_err(classified)?
        .items;
    if !catalog
        .is_unique_key(&id, std::slice::from_ref(&key_name))
        .await
        .map_err(classified)?
    {
        return Err(invalid());
    }
    let key = [(key_name, query.value)];
    let omitted_columns = columns.len() > 64;
    let mut values = Vec::with_capacity(columns.len().min(64));
    let mut total_bytes = 0usize;
    let mut truncated = omitted_columns;
    for column in columns.into_iter().take(64) {
        let result = catalog
            .cell_value(&id, &column.column.name, &key)
            .await
            .map_err(classified)?;
        total_bytes = total_bytes.saturating_add(result.text.as_ref().map_or(0, String::len));
        if total_bytes > 262_144 {
            truncated = true;
            break;
        }
        values.push(serde_json::json!({
            "name": column.column.name,
            "type": column.column.native_type,
            "value": result.text,
            "truncated": result.truncated,
        }));
    }
    let mut response = axum::Json(
        serde_json::json!({ "schema": id.namespace, "relation": id.name, "values": values, "truncated": truncated }),
    )
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{detail_value, value_class};
    use crate::db::types::Cell;

    #[test]
    fn rich_value_details_are_bounded_and_preserve_meaning() {
        assert_eq!(value_class("jsonb"), "json-value");
        assert_eq!(value_class("integer"), "numeric");
        assert_eq!(
            detail_value(&Cell::Text(r#"{"a":1}"#.into()), "jsonb"),
            "{\n  \"a\": 1\n}"
        );
        assert_eq!(
            detail_value(&Cell::Bytes(vec![0, 255]), "bytea"),
            r#"\x00ff"#
        );
        assert_eq!(detail_value(&Cell::Null, "text"), "NULL");
    }

    #[test]
    fn deeply_nested_and_oversized_json_stays_bounded_text() {
        // 500 levels exceed the JSON parser's recursion limit: shown as the raw text, no panic.
        let deep = format!("{}1{}", "[".repeat(500), "]".repeat(500));
        assert_eq!(detail_value(&Cell::Text(deep.clone()), "jsonb"), deep);
        // Valid nesting that pretty-prints beyond the 1 KiB detail budget also falls back.
        let wide = format!("[{}]", vec!["{\"k\":[1,2,3]}"; 400].join(","));
        assert_eq!(detail_value(&Cell::Text(wide.clone()), "json"), wide);
        // Small nested documents are pretty-printed for the sheet.
        let small = detail_value(&Cell::Text(r#"{"a":{"b":[1,2]}}"#.into()), "jsonb");
        assert!(small.contains('\n') && small.contains("\"b\""));
    }
}

#[cfg(test)]
mod visible_text_tests {
    use super::visible_text;

    #[test]
    fn controls_and_bidi_overrides_become_visible_escapes() {
        assert_eq!(visible_text("a\u{7}b"), "a\\u{0007}b");
        assert_eq!(visible_text("x\u{202E}y"), "x\\u{202E}y");
        assert_eq!(visible_text("\u{2066}z"), "\\u{2066}z");
        assert_eq!(visible_text("\u{0}"), "\\u{0000}");
    }

    #[test]
    fn ordinary_text_newlines_tabs_and_unicode_are_unchanged() {
        assert_eq!(
            visible_text("line1\n\tline2 Ünï 日本 😀"),
            "line1\n\tline2 Ünï 日本 😀"
        );
    }
}

/// `GET /catalog/cell?schema&relation&column&key.<pk>=value...`: the whole text of one cell
/// (capped at 64 K characters), addressed by the full primary key.
pub async fn cell(
    State(state): State<AppState>,
    auth: crate::web::auth::Auth,
    RawQuery(raw): RawQuery,
) -> Result<Response, AppError> {
    let mut schema = None;
    let mut relation = None;
    let mut column = None;
    let mut profile = None;
    let mut key: Vec<(String, String)> = Vec::new();
    for (name, value) in url::form_urlencoded::parse(raw.as_deref().unwrap_or("").as_bytes()) {
        let value = value.into_owned();
        match name.as_ref() {
            "schema" if schema.is_none() => schema = Some(value),
            "relation" if relation.is_none() => relation = Some(value),
            "column" if column.is_none() => column = Some(value),
            "profile" if profile.is_none() => profile = Some(value),
            other => match other.strip_prefix("key.") {
                Some(k) if !key.iter().any(|(n, _)| n == k) && key.len() < 8 => {
                    key.push((self::name(k)?, value));
                }
                _ => return Err(invalid()),
            },
        }
    }
    let id = RelationId {
        namespace: name(&schema.ok_or_else(invalid)?)?,
        name: name(&relation.ok_or_else(invalid)?)?,
    };
    let column = name(&column.ok_or_else(invalid)?)?;
    if key.is_empty()
        || key
            .iter()
            .any(|(_, v)| v.len() > 512 || v.chars().any(char::is_control))
    {
        return Err(invalid());
    }
    let profile = crate::state::checked_profile(profile)?;
    let pid = state.effective_profile(profile.as_deref());
    auth.authorize(
        &state,
        crate::auth::Action::View,
        &crate::auth::Resource::relation(&pid, &id.namespace, &id.name),
    )
    .await?;
    let value = state
        .adapter(profile.as_deref())?
        .cell_value(&id, &column, &key)
        .await
        .map_err(classified)?;
    let mut body = value.text.unwrap_or_default();
    if value.truncated {
        body.push_str("\n… (cut at 65,536 characters)");
    }
    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    headers.insert("x-cuadb-null", axum::http::HeaderValue::from_static("0"));
    Ok(response)
}
