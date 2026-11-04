//! Validated server-side row paging endpoint. Never accepts raw SQL.
use crate::{
    db::types::{DbError, DbErrorKind, RelationId, RowOptions, SortDirection, SortSpec},
    error::{AppError, ErrorKind},
    state::AppState,
};
use askama::Template;
use axum::{
    extract::{Query, State},
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
}
#[derive(Clone)]
struct Value {
    text: String,
    native_type: String,
    is_null: bool,
    truncated: bool,
    detail: String,
    class: &'static str,
}
#[derive(Clone)]
struct DisplayRow {
    number: u64,
    values: Vec<Value>,
}
#[derive(Template)]
#[template(path = "components/data_page.html")]
struct PageTemplate {
    title: String,
    filter: String,
    page_size: u32,
    has_spatial: bool,
    geojson_active: bool,
    geojson_url: String,
    sort: Option<String>,
    direction: Option<&'static str>,
    selected_columns: Vec<String>,
    available_columns: Vec<ColumnView>,
    compact: bool,
    schema: String,
    relation: String,
    headers: Vec<Header>,
    rows: Vec<DisplayRow>,
    previous_url: String,
    next_url: String,
    page_number: u32,
    size_25: String,
    size_50: String,
    size_100: String,
    stable: bool,
    truncated_columns: bool,
    shown_from: u64,
    shown_to: u64,
    approximate_total: String,
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
fn value_class(native_type: &str) -> &'static str {
    let native = native_type.to_ascii_lowercase();
    if native.contains("json") {
        "json-value"
    } else if native.ends_with("[]") {
        "array-value"
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
    #[serde(default)]
    pub(super) column: Vec<String>,
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
        limit: query.size.unwrap_or(25),
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
        || options.limit > 100
        || (options.page > 100 && options.cursor.is_none())
        || (options.cursor.is_none() && u64::from(options.page) * u64::from(options.limit) > 10_000)
    {
        return Err(invalid());
    }
    Ok(Navigation {
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
pub async fn data(
    State(state): State<AppState>,
    Query(query): Query<DataQuery>,
) -> Result<Response, AppError> {
    let nav = parse(query)?;
    let catalog = state
        .catalog()
        .ok_or(AppError::new(ErrorKind::DatabaseUnavailable))?;
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
    let has_spatial = page.columns.iter().any(|column| {
        let native = column.native_type.to_ascii_lowercase();
        native.starts_with("geometry") || native.starts_with("geography")
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
        let values = cells
            .iter()
            .zip(previews)
            .map(|(cell, preview)| {
                let mut detail = detail_value(cell, &preview.type_name);
                if let Some(spatial) = preview.detail.as_deref() {
                    detail.push_str("\n\nGeoJSON detail:\n");
                    detail.push_str(spatial);
                }
                Value {
                    text: preview.text,
                    class: value_class(&preview.type_name),
                    native_type: preview.type_name,
                    truncated: preview.truncated,
                    is_null: matches!(cell, crate::db::types::Cell::Null),
                    detail,
                }
            })
            .collect();
        rendered.push(DisplayRow {
            number: shown_from + index as u64,
            values,
        });
    }
    let mut sizes = nav.clone();
    sizes.options.limit = 25;
    let size_25 = link(&sizes, 0, None, sizes.options.sort.clone());
    sizes.options.limit = 50;
    let size_50 = link(&sizes, 0, None, sizes.options.sort.clone());
    sizes.options.limit = 100;
    let size_100 = link(&sizes, 0, None, sizes.options.sort.clone());
    let mut geojson_nav = nav.clone();
    geojson_nav.options.spatial_geojson = !nav.options.spatial_geojson;
    let geojson_url = link(&geojson_nav, 0, None, geojson_nav.options.sort.clone());
    let view = PageTemplate {
        title: format!("Data · {}.{}", nav.id.namespace, nav.id.name),
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
        compact: false,
        schema: url::form_urlencoded::byte_serialize(nav.id.namespace.as_bytes()).collect(),
        relation: url::form_urlencoded::byte_serialize(nav.id.name.as_bytes()).collect(),
        headers,
        rows: rendered,
        previous_url,
        next_url,
        page_number: page.page.saturating_add(1),
        size_25,
        size_50,
        size_100,
        stable: page.stable_order,
        truncated_columns: page.truncated_columns,
        shown_from,
        shown_to,
        approximate_total,
    };
    let body = view.render().map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
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
}
