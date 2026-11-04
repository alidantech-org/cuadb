//! Read-only, permission-aware catalog HTMX fragments.
//! The backend alone chooses SQL; requests can only select object identifiers.
use askama::Template;
use axum::{
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
};

use crate::{
    db::types::{
        Availability, CatalogList, ColumnDetail, Constraint, DbError, DbErrorKind, Index,
        PartitionLink, RelationId, RelationStats,
    },
    error::{AppError, ErrorKind},
    state::AppState,
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
}
#[derive(Clone)]
struct SchemaItem {
    name: String,
    href: String,
}
#[derive(Template)]
#[template(path = "components/relation_tree.html")]
struct RelationTreeTemplate {
    schema: String,
    relations: Vec<RelationItem>,
    note: String,
}
struct RelationItem {
    name: String,
    kind: String,
    href: String,
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
        DbErrorKind::Internal => ErrorKind::Internal,
    })
}
fn catalog(
    state: &AppState,
) -> Result<std::sync::Arc<dyn crate::db::adapter::DatabaseAdapter>, AppError> {
    state
        .catalog()
        .ok_or(AppError::new(ErrorKind::DatabaseUnavailable))
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
fn relation_links(id: &RelationId) -> RelationLinks {
    RelationLinks {
        data: relation_url("data", id),
        structure: relation_url("structure", id),
        indexes: relation_url("indexes", id),
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
fn relation_url(route: &str, id: &RelationId) -> String {
    format!(
        "/catalog/{route}?schema={}&relation={}",
        url_arg(&id.namespace),
        url_arg(&id.name)
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
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationQuery {
    schema: String,
    relation: String,
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

pub async fn schemas(State(state): State<AppState>) -> Result<Response, AppError> {
    let Some(c) = state.catalog() else {
        return tree_response(SchemaTreeTemplate {
            schemas: vec![],
            note: "Not configured. Set CUADB_DATABASE_URL on the server to browse the catalog."
                .into(),
        });
    };
    let entries = match c.list_schemas().await {
        Ok(entries) => entries,
        Err(db) => {
            return tree_response(SchemaTreeTemplate {
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
    let schemas = entries
        .items
        .into_iter()
        .map(|schema| SchemaItem {
            href: format!("/catalog/relations?schema={}", url_arg(&schema.name)),
            name: schema.name,
        })
        .collect();
    tree_response(SchemaTreeTemplate {
        schemas,
        note: if truncated {
            "Only schemas with USAGE are listed; the bounded list is incomplete.".into()
        } else {
            "Only schemas with USAGE privileges are listed; empty schemas remain visible.".into()
        },
    })
}
pub async fn relations(
    State(state): State<AppState>,
    Query(query): Query<SchemaQuery>,
) -> Result<Response, AppError> {
    let schema = checked_query_name(query.schema)?;
    let entries = catalog(&state)?
        .list_relations(&schema)
        .await
        .map_err(err)?;
    let truncated = entries.truncated;
    let relations = entries
        .items
        .into_iter()
        .map(|relation| RelationItem {
            name: relation.id.name.clone(),
            kind: format!("{:?}", relation.kind),
            href: relation_url("data", &relation.id),
        })
        .collect();
    tree_response(RelationTreeTemplate {
        schema,
        relations,
        note: if truncated {
            "Only SELECT-visible relations are listed; the bounded list is incomplete.".into()
        } else {
            "Only relations with SELECT privilege are listed.".into()
        },
    })
}
pub async fn structure(
    State(state): State<AppState>,
    Query(query): Query<RelationQuery>,
) -> Result<Response, AppError> {
    let id = RelationId {
        namespace: checked_query_name(query.schema)?,
        name: checked_query_name(query.relation)?,
    };
    let c = catalog(&state)?;
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
        .map(|column| structure_column(column))
        .collect::<Vec<_>>();
    columns.sort_by_key(|column| column.position);
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
        title: format!("Structure · {}.{}", id.namespace, id.name),
        links: relation_links(&id),
        columns,
        constraints,
        partitions,
        truncated,
    })
}
pub async fn indexes(
    State(state): State<AppState>,
    Query(query): Query<RelationQuery>,
) -> Result<Response, AppError> {
    let id = RelationId {
        namespace: checked_query_name(query.schema)?,
        name: checked_query_name(query.relation)?,
    };
    let c = catalog(&state)?;
    let (entries, statistics) =
        tokio::try_join!(c.list_indexes(&id), c.relation_statistics(&id)).map_err(err)?;
    let truncated = entries.truncated;
    let indexes = entries.items.into_iter().map(index_view).collect();
    metadata_response(IndexesTemplate {
        title: format!("Indexes · {}.{}", id.namespace, id.name),
        links: relation_links(&id),
        indexes,
        statistics: statistic_views(&statistics),
        truncated,
    })
}
pub async fn stats(
    State(state): State<AppState>,
    Query(query): Query<RelationQuery>,
) -> Result<Response, AppError> {
    let id = RelationId {
        namespace: checked_query_name(query.schema)?,
        name: checked_query_name(query.relation)?,
    };
    let s = catalog(&state)?
        .relation_statistics(&id)
        .await
        .map_err(err)?;
    metadata_response(IndexesTemplate {
        title: format!("Indexes and storage · {}.{}", id.namespace, id.name),
        links: relation_links(&id),
        indexes: Vec::new(),
        statistics: statistic_views(&s),
        truncated: false,
    })
}
pub async fn advanced(
    State(state): State<AppState>,
    Query(query): Query<SchemaQuery>,
) -> Result<Response, AppError> {
    let schema = checked_query_name(query.schema)?;
    let entries = catalog(&state)?.list_advanced(&schema).await.map_err(err)?;
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
pub async fn overview(State(state): State<AppState>) -> Result<Response, AppError> {
    let o = catalog(&state)?.advanced_overview().await.map_err(err)?;
    let mut rows = vec![
        row("Visible schemas", o.visible_schema_count.to_string(), ""),
        row(
            "Visible relations",
            o.visible_relation_count.to_string(),
            "",
        ),
    ];
    rows.extend(
        o.extensions
            .into_iter()
            .map(|e| row(format!("Extension · {e}"), "Installed", "")),
    );
    rows.extend(o.schema_sizes.items.into_iter().map(|s| {
        row(
            format!("Schema · {}", s.namespace),
            format!("Estimated bytes: {}", availability(&s.estimated_bytes)),
            "",
        )
    }));
    rows.extend(o.largest_relations.items.into_iter().map(|(r, size)| {
        row(
            format!("{}.{}", r.namespace, r.name),
            format!("Estimated bytes: {}", availability(&size)),
            relation_url("structure", &r),
        )
    }));
    fragment(
        "Advanced overview".into(),
        rows,
        o.truncated,
        "Overview size estimates use catalog relpages. Counts and charts can be truncated.",
    )
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
            relation_url("structure", &id),
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
