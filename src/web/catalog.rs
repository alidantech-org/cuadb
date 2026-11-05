//! Read-only, permission-aware catalog HTMX fragments.
//! The backend alone chooses SQL; requests can only select object identifiers.
use askama::Template;
use axum::{
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
};

use crate::{
    auth::{Action, Resource},
    db::types::{
        Availability, CatalogList, ColumnDetail, Constraint, DbError, DbErrorKind, Index,
        PartitionLink, RelationId, RelationStats,
    },
    error::{AppError, ErrorKind},
    state::{AppState, Lookup, checked_profile},
    web::auth::Auth,
};

#[derive(Clone)]
struct DisplayRow {
    title: String,
    description: String,
    href: String,
}

#[derive(Template)]
#[template(path = "components/catalog.html")]
struct CatalogTemplate {
    title: String,
    rows: Vec<DisplayRow>,
    truncated: bool,
    note: &'static str,
    relation: Option<RelationLinks>,
    data_active: bool,
    structure_active: bool,
    indexes_active: bool,
}
struct RelationLinks {
    data: String,
    structure: String,
    indexes: String,
}
#[derive(Template)]
#[template(path = "components/structure_page.html")]
struct StructureTemplate {
    title: String,
    active: &'static str,
    links: RelationLinks,
    columns: Vec<StructureColumn>,
    constraints: Vec<StructureConstraint>,
    partitions: Vec<StructurePartition>,
    truncated: bool,
}
struct StructureColumn {
    position: i16,
    name: String,
    native_type: String,
    nullable: bool,
    key: String,
    generation: String,
    /// `schema.table.column` of a single-column foreign key, shown as a link.
    references: String,
    ref_href: String,
}
struct StructureConstraint {
    name: String,
    kind: String,
    columns: String,
    references: String,
    definition: String,
}
struct StructurePartition {
    parent: String,
    child: String,
    bound: String,
}
#[derive(Template)]
#[template(path = "components/indexes_page.html")]
struct IndexesTemplate {
    title: String,
    active: &'static str,
    links: RelationLinks,
    indexes: Vec<IndexView>,
    statistics: Vec<StatisticView>,
    truncated: bool,
}
struct IndexView {
    name: String,
    kind: String,
    columns: String,
    valid: bool,
    predicate: String,
    definition: String,
    unique: bool,
}
struct StatisticView {
    name: String,
    value: String,
    meaning: String,
}

#[derive(Template)]
#[template(path = "components/schema_tree.html")]
struct SchemaTreeTemplate {
    schemas: Vec<SchemaItem>,
    note: String,
    overview_href: String,
}
#[derive(Clone)]
struct SchemaItem {
    name: String,
    href: String,
}
#[derive(Template)]
#[template(path = "components/relation_tree.html")]
struct RelationTreeTemplate {
    grouped: bool,
    table_count: usize,
    view_count: usize,
    schema: String,
    objects_href: String,
    relations: Vec<RelationItem>,
    note: String,
    /// The bounded catalog list was cut off; tell the user even when rows exist.
    truncated: bool,
}
struct RelationItem {
    name: String,
    kind: String,
    href: String,
    is_view: bool,
}

fn err(db: DbError) -> AppError {
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
fn catalog(
    state: &AppState,
    profile: Option<&str>,
) -> Result<std::sync::Arc<dyn crate::db::adapter::DatabaseAdapter>, AppError> {
    state.adapter(profile)
}
/// `&profile=<id>` suffix keeping navigation on the selected profile.
fn profile_arg(profile: Option<&str>) -> String {
    profile.map_or(String::new(), |id| format!("&profile={}", url_arg(id)))
}
fn fragment(
    title: String,
    rows: Vec<DisplayRow>,
    truncated: bool,
    note: &'static str,
) -> Result<Response, AppError> {
    let body = CatalogTemplate {
        title,
        rows,
        truncated,
        note,
        relation: None,
        data_active: false,
        structure_active: false,
        indexes_active: false,
    }
    .render()
    .map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
fn relation_links(id: &RelationId, profile: Option<&str>) -> RelationLinks {
    RelationLinks {
        data: relation_url("data", id, profile),
        structure: relation_url("structure", id, profile),
        indexes: relation_url("indexes", id, profile),
    }
}
fn metadata_response<T: Template>(view: T) -> Result<Response, AppError> {
    tree_response(view)
}
fn statistic_views(stats: &RelationStats) -> Vec<StatisticView> {
    let relation_kind = match &stats.relation_kind {
        Availability::Available(kind) => format!("{kind:?}"),
        Availability::Unavailable => "Unavailable".into(),
    };
    vec![
        StatisticView {
            name: "Relation type".into(),
            value: relation_kind,
            meaning: "Database-reported relation classification".into(),
        },
        StatisticView {
            name: "Estimated rows".into(),
            value: availability(&stats.estimated_rows),
            meaning: "Engine-reported estimate; not an exact COUNT(*)".into(),
        },
        StatisticView {
            name: "Table data size".into(),
            value: availability(&stats.table_bytes),
            meaning: "Engine-reported bytes when available; may be approximate or stale".into(),
        },
        StatisticView {
            name: "Index size".into(),
            value: availability(&stats.index_bytes),
            meaning: "Engine-reported bytes when available; may be approximate or stale".into(),
        },
        StatisticView {
            name: "Total size".into(),
            value: availability(&stats.total_bytes),
            meaning: "Engine-reported combined bytes when available".into(),
        },
    ]
}
fn tree_response<T: Template>(view: T) -> Result<Response, AppError> {
    let body = view.render().map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
fn row(
    title: impl Into<String>,
    description: impl Into<String>,
    href: impl Into<String>,
) -> DisplayRow {
    DisplayRow {
        title: title.into(),
        description: description.into(),
        href: href.into(),
    }
}
fn url_arg(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
fn relation_url(route: &str, id: &RelationId, profile: Option<&str>) -> String {
    format!(
        "/catalog/{route}?schema={}&relation={}{}",
        url_arg(&id.namespace),
        url_arg(&id.name),
        profile_arg(profile)
    )
}
fn availability<T: std::fmt::Display>(value: &Availability<T>) -> String {
    match value {
        Availability::Available(n) => n.to_string(),
        Availability::Unavailable => "Unavailable".to_string(),
    }
}
fn structure_column(detail: ColumnDetail) -> StructureColumn {
    let column = detail.column;
    let mut native_type = column.native_type;
    if let Some(domain) = detail.domain {
        native_type.push_str(&format!(" · domain {domain}"));
    }
    if let Availability::Available(labels) = detail.enum_values {
        native_type.push_str(&format!(" · enum ({})", labels.join(", ")));
    }
    let key = if column.primary_key {
        "Primary key"
    } else if detail.unique_key {
        "Unique key"
    } else {
        "—"
    };
    let mut generation = Vec::new();
    if let Some(identity) = detail.identity {
        generation.push(format!("Identity {identity}"));
    }
    if let Some(generated) = detail.generated {
        generation.push(format!("Generated {generated}"));
    }
    if let Some(default) = column.default_expression {
        generation.push(format!("Default {default}"));
    }
    StructureColumn {
        position: detail.position,
        name: column.name,
        native_type,
        nullable: column.nullable,
        key: key.into(),
        generation: if generation.is_empty() {
            "—".into()
        } else {
            generation.join(" · ")
        },
        references: String::new(),
        ref_href: String::new(),
    }
}
fn structure_constraint(constraint: Constraint) -> StructureConstraint {
    let references = constraint.references.map_or_else(
        || "—".to_string(),
        |relation| {
            format!(
                "{}.{} ({})",
                relation.namespace,
                relation.name,
                constraint.referenced_columns.join(", ")
            )
        },
    );
    StructureConstraint {
        name: constraint.name,
        kind: format!("{:?}", constraint.kind),
        columns: constraint.columns.join(", "),
        references,
        definition: constraint.definition,
    }
}
fn structure_partition(partition: PartitionLink) -> StructurePartition {
    StructurePartition {
        parent: format!("{}.{}", partition.parent.namespace, partition.parent.name),
        child: format!("{}.{}", partition.child.namespace, partition.child.name),
        bound: partition.bound.unwrap_or_else(|| "Unavailable".into()),
    }
}
fn index_view(index: Index) -> IndexView {
    IndexView {
        name: index.name,
        kind: if index.primary {
            "Primary".into()
        } else if index.unique {
            "Unique".into()
        } else {
            "Index".into()
        },
        columns: index.columns_or_expressions.join(", "),
        valid: index.valid,
        predicate: index.predicate.unwrap_or_else(|| "—".into()),
        definition: index.definition,
        unique: index.unique || index.primary,
    }
}
fn validate_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    Ok(())
}
#[cfg(test)]
fn params(raw: Option<String>, relation: bool) -> Result<(String, Option<String>), AppError> {
    let raw = raw.ok_or(AppError::new(ErrorKind::InvalidInput))?;
    if raw.len() > 512 {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    let mut schema: Option<String> = None;
    let mut name: Option<String> = None;
    for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
        validate_name(&v)?;
        match k.as_ref() {
            "schema" if schema.is_none() => schema = Some(v.into_owned()),
            "relation" if name.is_none() => name = Some(v.into_owned()),
            _ => return Err(AppError::new(ErrorKind::InvalidInput)),
        }
    }
    let schema = schema.ok_or(AppError::new(ErrorKind::InvalidInput))?;
    if relation && name.is_none() {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    if !relation && name.is_some() {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    Ok((schema, name))
}
#[cfg(test)]
fn relation(raw: Option<String>) -> Result<RelationId, AppError> {
    let (namespace, name) = params(raw, true)?;
    Ok(RelationId {
        namespace,
        name: name.expect("checked above"),
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaQuery {
    schema: String,
    /// Optional literal, case-insensitive name filter (no wildcards).
    q: Option<String>,
    /// Trusted connection profile id; absent means the default profile.
    profile: Option<String>,
    /// Advanced explorer: group tables and views under headings.
    group: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationQuery {
    schema: String,
    relation: String,
    profile: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileQuery {
    pub(crate) profile: Option<String>,
}

fn checked_query_name(value: String) -> Result<String, AppError> {
    validate_name(&value)?;
    Ok(value)
}

fn visible<T, F>(items: CatalogList<T>, f: F) -> (Vec<DisplayRow>, bool)
where
    F: Fn(T) -> DisplayRow,
{
    (items.items.into_iter().map(f).collect(), items.truncated)
}

pub async fn schemas(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<ProfileQuery>,
) -> Result<Response, AppError> {
    let profile = checked_profile(query.profile)?;
    let profile = profile.as_deref();
    let pid = state.effective_profile(profile);
    let c = match state.catalog_for(profile) {
        Lookup::Found(c) => {
            auth.authorize(&state, Action::View, &Resource::profile(&pid))
                .await?;
            crate::auth::audit_profile_access(&state, &auth.0, &pid).await;
            c
        }
        Lookup::Unknown => return Err(AppError::new(ErrorKind::UnknownProfile)),
        Lookup::NotConfigured => {
            return tree_response(SchemaTreeTemplate {
                overview_href: String::new(),
                schemas: vec![],
                note: "Not configured. Set CUADB_DATABASE_URL or CUADB_PROFILES_FILE on the server to browse the catalog."
                    .into(),
            });
        }
    };
    let entries = match c.list_schemas().await {
        Ok(entries) => entries,
        Err(db) => {
            return tree_response(SchemaTreeTemplate {
                overview_href: String::new(),
                schemas: vec![],
                note: match db.kind {
                    DbErrorKind::PermissionDenied => {
                        "Insufficient permissions to read the catalog.".into()
                    }
                    DbErrorKind::TimedOut => "The database catalog did not respond in time.".into(),
                    _ => "The configured database is unavailable.".into(),
                },
            });
        }
    };
    let truncated = entries.truncated;
    let schemas: Vec<SchemaItem> = entries
        .items
        .into_iter()
        .filter(|schema| auth.0.can_see_schema(&pid, &schema.name))
        .map(|schema| SchemaItem {
            href: format!(
                "/catalog/relations?schema={}{}",
                url_arg(&schema.name),
                profile_arg(profile)
            ),
            name: schema.name,
        })
        .collect();
    let schemas_empty = schemas.is_empty();
    tree_response(SchemaTreeTemplate {
        overview_href: format!(
            "/catalog/overview?{}",
            profile_arg(profile).trim_start_matches('&')
        ),
        schemas,
        note: if truncated {
            "Only schemas with USAGE are listed; the bounded list is incomplete.".into()
        } else if !auth.0.is_open() && schemas_empty {
            "No databases assigned yet. Ask an administrator for access.".into()
        } else {
            "Only schemas with USAGE privileges are listed; empty schemas remain visible.".into()
        },
    })
}
pub async fn relations(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<SchemaQuery>,
) -> Result<Response, AppError> {
    let schema = checked_query_name(query.schema)?;
    let profile = checked_profile(query.profile)?;
    let profile = profile.as_deref();
    let pid = state.effective_profile(profile);
    if !auth.0.can_see_schema(&pid, &schema) {
        auth.denied(&state, Action::View, &format!("{pid}/{schema}"))
            .await;
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let needle = match query.q {
        Some(q) if q.is_empty() => None,
        Some(q) if q.chars().count() > 64 || q.chars().any(char::is_control) => {
            return Err(AppError::new(ErrorKind::InvalidInput));
        }
        Some(q) => Some(q.to_lowercase()),
        None => None,
    };
    let entries = catalog(&state, profile)?
        .list_relations(&schema)
        .await
        .map_err(err)?;
    let truncated = entries.truncated;
    let relations = entries
        .items
        .into_iter()
        .filter(|relation| {
            auth.0.allows(
                Action::View,
                &Resource::relation(&pid, &schema, &relation.id.name),
            )
        })
        .filter(|relation| {
            needle
                .as_ref()
                .is_none_or(|n| relation.id.name.to_lowercase().contains(n))
        })
        .map(|relation| {
            let kind = format!("{:?}", relation.kind);
            RelationItem {
                name: relation.id.name.clone(),
                is_view: kind.contains("View"),
                kind,
                href: relation_url("data", &relation.id, profile),
            }
        })
        .collect::<Vec<_>>();
    let view_count = relations.iter().filter(|r| r.is_view).count();
    tree_response(RelationTreeTemplate {
        grouped: query.group.as_deref() == Some("1"),
        table_count: relations.len() - view_count,
        view_count,
        objects_href: format!(
            "/catalog/objects?schema={}{}",
            url_arg(&schema),
            profile_arg(profile)
        ),
        schema,
        relations,
        truncated,
        note: if truncated {
            "Only SELECT-visible relations are listed; the bounded list (and any search over it) is incomplete.".into()
        } else if needle.is_some() {
            "Filtered by name among SELECT-visible relations.".into()
        } else {
            "Only relations with SELECT privilege are listed.".into()
        },
    })
}
pub async fn structure(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<RelationQuery>,
) -> Result<Response, AppError> {
    let id = RelationId {
        namespace: checked_query_name(query.schema)?,
        name: checked_query_name(query.relation)?,
    };
    let profile = checked_profile(query.profile)?;
    let profile = profile.as_deref();
    let pid = state.effective_profile(profile);
    auth.authorize(
        &state,
        Action::View,
        &Resource::relation(&pid, &id.namespace, &id.name),
    )
    .await?;
    let c = catalog(&state, profile)?;
    let (cols, constraints, partitions) = tokio::try_join!(
        c.describe_columns(&id),
        c.list_constraints(&id),
        c.list_partitions(&id),
    )
    .map_err(err)?;
    let truncated = cols.truncated || constraints.truncated || partitions.truncated;
    let mut columns = cols
        .items
        .into_iter()
        .map(structure_column)
        .collect::<Vec<_>>();
    columns.sort_by_key(|column| column.position);
    // Single-column foreign keys become the column's "References" link.
    for constraint in &constraints.items {
        if constraint.kind != crate::db::types::ConstraintKind::ForeignKey
            || constraint.columns.len() != 1
        {
            continue;
        }
        let (Some(target), Some(column)) = (
            constraint.references.as_ref(),
            columns.iter_mut().find(|c| c.name == constraint.columns[0]),
        ) else {
            continue;
        };
        column.references = format!(
            "{}.{}{}",
            target.namespace,
            target.name,
            constraint
                .referenced_columns
                .first()
                .map_or(String::new(), |c| format!(".{c}"))
        );
        column.ref_href = relation_url("data", target, profile);
    }
    let constraints = constraints
        .items
        .into_iter()
        .map(structure_constraint)
        .collect();
    let partitions = partitions
        .items
        .into_iter()
        .map(structure_partition)
        .collect();
    metadata_response(StructureTemplate {
        active: "structure",
        title: format!("Structure · {}.{}", id.namespace, id.name),
        links: relation_links(&id, profile),
        columns,
        constraints,
        partitions,
        truncated,
    })
}
/// Relations in the same schema whose foreign keys point here. Bounded to the first 50
/// visible relations, so it is a hint, not a complete dependency graph.
async fn referenced_by(
    c: &std::sync::Arc<dyn crate::db::adapter::DatabaseAdapter>,
    id: &RelationId,
    auth: &Auth,
    pid: &str,
) -> StatisticView {
    let meaning =
        "Same-schema foreign keys that point at this relation (first 50 visible relations)"
            .to_string();
    let Ok(list) = c.list_relations(&id.namespace).await else {
        return StatisticView {
            name: "Referenced by".into(),
            value: "Unavailable".into(),
            meaning,
        };
    };
    let mut found = Vec::new();
    for relation in list.items.into_iter().take(50) {
        if relation.id == *id
            || !auth.0.allows(
                Action::View,
                &Resource::relation(pid, &relation.id.namespace, &relation.id.name),
            )
        {
            continue;
        }
        let Ok(constraints) = c.list_constraints(&relation.id).await else {
            continue;
        };
        if constraints.items.iter().any(|k| {
            k.kind == crate::db::types::ConstraintKind::ForeignKey
                && k.references.as_ref() == Some(id)
        }) {
            found.push(relation.id.name);
        }
    }
    StatisticView {
        name: "Referenced by".into(),
        value: if found.is_empty() {
            "—".into()
        } else {
            found.join(", ")
        },
        meaning,
    }
}
pub async fn indexes(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<RelationQuery>,
) -> Result<Response, AppError> {
    let id = RelationId {
        namespace: checked_query_name(query.schema)?,
        name: checked_query_name(query.relation)?,
    };
    let profile = checked_profile(query.profile)?;
    let profile = profile.as_deref();
    let pid = state.effective_profile(profile);
    auth.authorize(
        &state,
        Action::View,
        &Resource::relation(&pid, &id.namespace, &id.name),
    )
    .await?;
    let c = catalog(&state, profile)?;
    let (entries, statistics) =
        tokio::try_join!(c.list_indexes(&id), c.relation_statistics(&id)).map_err(err)?;
    let truncated = entries.truncated;
    let indexes = entries.items.into_iter().map(index_view).collect();
    let mut statistics = statistic_views(&statistics);
    statistics.push(referenced_by(&c, &id, &auth, &pid).await);
    metadata_response(IndexesTemplate {
        active: "indexes",
        title: format!("Indexes · {}.{}", id.namespace, id.name),
        links: relation_links(&id, profile),
        indexes,
        statistics,
        truncated,
    })
}
pub async fn stats(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<RelationQuery>,
) -> Result<Response, AppError> {
    let id = RelationId {
        namespace: checked_query_name(query.schema)?,
        name: checked_query_name(query.relation)?,
    };
    let profile = checked_profile(query.profile)?;
    let profile = profile.as_deref();
    let pid = state.effective_profile(profile);
    auth.authorize(
        &state,
        Action::View,
        &Resource::relation(&pid, &id.namespace, &id.name),
    )
    .await?;
    let s = catalog(&state, profile)?
        .relation_statistics(&id)
        .await
        .map_err(err)?;
    metadata_response(IndexesTemplate {
        active: "indexes",
        title: format!("Indexes and storage · {}.{}", id.namespace, id.name),
        links: relation_links(&id, profile),
        indexes: Vec::new(),
        statistics: statistic_views(&s),
        truncated: false,
    })
}
pub async fn advanced(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<SchemaQuery>,
) -> Result<Response, AppError> {
    let schema = checked_query_name(query.schema)?;
    let profile = checked_profile(query.profile)?;
    let pid = state.effective_profile(profile.as_deref());
    if !auth.0.allows_wide(Action::View, &pid, Some(&schema)) {
        auth.denied(&state, Action::View, &format!("{pid}/{schema} (advanced)"))
            .await;
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let entries = catalog(&state, profile.as_deref())?
        .list_advanced(&schema)
        .await
        .map_err(err)?;
    let (rows, truncated) = visible(entries, |o| {
        row(
            format!("{:?} · {}", o.kind, o.name),
            match o.definition {
                Availability::Available(d) => d,
                Availability::Unavailable => "Definition unavailable".into(),
            },
            String::new(),
        )
    });
    fragment(
        format!("Advanced objects · {schema}"),
        rows,
        truncated,
        "Functions, procedures, sequences, types and triggers are metadata only; never executed.",
    )
}
fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{} {}", value.round() as u64, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

struct Bar {
    label: String,
    href: String,
    pct: u32,
    size: String,
}

fn bars(items: Vec<(String, String, Availability<u64>)>) -> Vec<Bar> {
    let known = |a: &Availability<u64>| match a {
        Availability::Available(n) => *n,
        Availability::Unavailable => 0,
    };
    let max = items
        .iter()
        .map(|(_, _, a)| known(a))
        .max()
        .unwrap_or(0)
        .max(1);
    items
        .into_iter()
        .map(|(label, href, size)| Bar {
            pct: ((known(&size) as f64 / max as f64) * 100.0)
                .round()
                .clamp(1.0, 100.0) as u32,
            size: match &size {
                Availability::Available(n) => human_bytes(*n),
                Availability::Unavailable => "unavailable".into(),
            },
            label,
            href,
        })
        .collect()
}

#[derive(Template)]
#[template(path = "components/overview.html")]
struct OverviewTemplate {
    schemas: usize,
    relations: usize,
    extensions: Vec<String>,
    schema_bars: Vec<Bar>,
    largest: Vec<Bar>,
    truncated: bool,
}

pub async fn overview(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<ProfileQuery>,
) -> Result<Response, AppError> {
    let profile = checked_profile(query.profile)?;
    let profile = profile.as_deref();
    let pid = state.effective_profile(profile);
    if !auth.0.allows_wide(Action::View, &pid, None) {
        auth.denied(&state, Action::View, &format!("{pid} (overview)"))
            .await;
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let o = catalog(&state, profile)?
        .advanced_overview()
        .await
        .map_err(err)?;
    tree_response(OverviewTemplate {
        schemas: o.visible_schema_count,
        relations: o.visible_relation_count,
        extensions: o.extensions,
        schema_bars: bars(
            o.schema_sizes
                .items
                .into_iter()
                .map(|s| (s.namespace, String::new(), s.estimated_bytes))
                .collect(),
        ),
        largest: bars(
            o.largest_relations
                .items
                .into_iter()
                .take(8)
                .map(|(r, size)| {
                    (
                        format!("{}.{}", r.namespace, r.name),
                        relation_url("data", &r, profile),
                        size,
                    )
                })
                .collect(),
        ),
        truncated: o.truncated,
    })
}

fn kind_code(kind: crate::db::types::CatalogObjectKind) -> &'static str {
    use crate::db::types::CatalogObjectKind as K;
    match kind {
        K::Function => "function",
        K::Procedure => "procedure",
        K::Sequence => "sequence",
        K::UserType => "type",
        K::Trigger => "trigger",
    }
}

fn kind_group(code: &str) -> &'static str {
    match code {
        "function" => "Functions",
        "procedure" => "Procedures",
        "sequence" => "Sequences",
        "type" => "Types",
        _ => "Triggers",
    }
}

struct ObjItem {
    name: String,
    kind: &'static str,
    href: String,
}
struct ObjGroup {
    label: &'static str,
    items: Vec<ObjItem>,
}

#[derive(Template)]
#[template(path = "components/object_tree.html")]
struct ObjectTreeTemplate {
    groups: Vec<ObjGroup>,
    truncated: bool,
}

/// Advanced explorer: metadata objects of a schema, grouped by kind, for the sidebar.
/// Engines without these objects return an empty fragment rather than an error.
pub async fn objects(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<SchemaQuery>,
) -> Result<Response, AppError> {
    let schema = checked_query_name(query.schema)?;
    let profile = checked_profile(query.profile)?;
    let pid = state.effective_profile(profile.as_deref());
    if !auth.0.allows_wide(Action::View, &pid, Some(&schema)) {
        auth.denied(&state, Action::View, &format!("{pid}/{schema} (objects)"))
            .await;
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let entries = match catalog(&state, profile.as_deref())?
        .list_advanced(&schema)
        .await
    {
        Ok(entries) => entries,
        Err(db) if db.kind == DbErrorKind::Unsupported => {
            return tree_response(ObjectTreeTemplate {
                groups: Vec::new(),
                truncated: false,
            });
        }
        Err(db) => return Err(err(db)),
    };
    let truncated = entries.truncated;
    let mut groups: Vec<ObjGroup> = ["function", "procedure", "sequence", "type", "trigger"]
        .into_iter()
        .map(|code| ObjGroup {
            label: kind_group(code),
            items: Vec::new(),
        })
        .collect();
    for object in entries.items {
        let code = kind_code(object.kind);
        let Some(group) = groups.iter_mut().find(|g| g.label == kind_group(code)) else {
            continue;
        };
        group.items.push(ObjItem {
            href: format!(
                "/catalog/object?schema={}&kind={code}&name={}{}",
                url_arg(&schema),
                url_arg(&object.name),
                profile_arg(profile.as_deref())
            ),
            kind: code,
            name: object.name,
        });
    }
    groups.retain(|g| !g.items.is_empty());
    tree_response(ObjectTreeTemplate { groups, truncated })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectQuery {
    schema: String,
    kind: String,
    name: String,
    profile: Option<String>,
}

#[derive(Template)]
#[template(path = "components/object_page.html")]
struct ObjectPageTemplate {
    schema: String,
    name: String,
    kind: &'static str,
    definition: String,
}

/// One metadata object: its definition text, shown as inert text and never executable.
pub async fn object(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<ObjectQuery>,
) -> Result<Response, AppError> {
    let schema = checked_query_name(query.schema)?;
    let name = checked_query_name(query.name)?;
    let kind = ["function", "procedure", "sequence", "type", "trigger"]
        .into_iter()
        .find(|code| *code == query.kind)
        .ok_or(AppError::new(ErrorKind::InvalidInput))?;
    let profile = checked_profile(query.profile)?;
    let pid = state.effective_profile(profile.as_deref());
    if !auth.0.allows_wide(Action::View, &pid, Some(&schema)) {
        auth.denied(&state, Action::View, &format!("{pid}/{schema} (object)"))
            .await;
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let entries = catalog(&state, profile.as_deref())?
        .list_advanced(&schema)
        .await
        .map_err(err)?;
    let found = entries
        .items
        .into_iter()
        .find(|o| kind_code(o.kind) == kind && o.name == name)
        .ok_or(AppError::new(ErrorKind::InvalidInput))?;
    tree_response(ObjectPageTemplate {
        schema,
        name: found.name,
        kind,
        definition: match found.definition {
            Availability::Available(text) => text,
            Availability::Unavailable => "Definition unavailable.".into(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_and_duplicate_navigation_parameters() {
        for q in [
            "schema=",
            "schema=public&schema=private",
            "schema=public&host=evil",
            "schema=public&relation=t&relation=t",
            "schema=%00",
        ] {
            assert!(params(Some(q.into()), false).is_err());
        }
        assert!(params(Some("schema=Mixed+Case".into()), false).is_ok());
        assert!(relation(Some("schema=public&relation=Mixed%20Name".into())).is_ok());
    }
    #[test]
    fn catalog_content_is_html_escaped() {
        let html = CatalogTemplate {
            title: "Test".into(),
            rows: vec![row("<img src=x onerror=alert(1)>", "value <script>", "")],
            truncated: false,
            note: "",
            relation: None,
            data_active: false,
            structure_active: false,
            indexes_active: false,
        }
        .render()
        .unwrap();
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img src=x"));
        assert!(html.contains("script"));
    }
    #[test]
    fn encodes_identifier_links_without_sql_or_secrets() {
        let id = RelationId {
            namespace: "A & B".into(),
            name: "x/y?".into(),
        };
        assert_eq!(
            relation_url("structure", &id, None),
            "/catalog/structure?schema=A+%26+B&relation=x%2Fy%3F"
        );
    }

    #[tokio::test]
    async fn typed_queries_reject_duplicates_unknown_fields_and_bad_names() {
        use axum::extract::FromRequestParts;
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };

        async fn parses<T>(uri: &str) -> Result<T, StatusCode>
        where
            T: serde::de::DeserializeOwned,
        {
            let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
            let (mut parts, _) = request.into_parts();
            Query::<T>::from_request_parts(&mut parts, &())
                .await
                .map(|Query(value)| value)
                .map_err(|rejection| rejection.status())
        }

        assert!(
            parses::<SchemaQuery>("/catalog/relations?schema=public&schema=other")
                .await
                .is_err()
        );
        assert!(
            parses::<SchemaQuery>("/catalog/relations?schema=public&url=http%3A%2F%2Fevil")
                .await
                .is_err()
        );
        let bad: RelationQuery = parses("/catalog/structure?schema=public&relation=%00")
            .await
            .unwrap();
        assert!(checked_query_name(bad.relation).is_err());
    }
}
