//! Read-only, permission-aware catalog HTMX fragments.
//! The backend alone chooses SQL; requests can only select object identifiers.
use askama::Template;
use axum::{
    extract::{RawQuery, State},
    http::{header, HeaderValue},
    response::{Html, IntoResponse, Response},
};

use crate::{
    db::{
        postgres::PgCatalog,
        types::{Availability, CatalogList, DbError, DbErrorKind, RelationId},
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
}

fn err(db: DbError) -> AppError {
    AppError::new(match db.kind {
        DbErrorKind::InvalidIdentifier => ErrorKind::InvalidInput,
        DbErrorKind::PermissionDenied => ErrorKind::Forbidden,
        DbErrorKind::TimedOut => ErrorKind::TimedOut,
        DbErrorKind::ConnectionUnavailable | DbErrorKind::AuthenticationFailed =>
            ErrorKind::DatabaseUnavailable,
        DbErrorKind::Unsupported => ErrorKind::InvalidInput,
        DbErrorKind::Internal => ErrorKind::Internal,
    })
}
fn catalog(state: &AppState) -> Result<PgCatalog, AppError> {
    state.catalog().ok_or(AppError::new(ErrorKind::DatabaseUnavailable))
}
fn fragment(title: String, rows: Vec<DisplayRow>, truncated: bool, note: &'static str)
    -> Result<Response, AppError>
{
    let body = CatalogTemplate { title, rows, truncated, note }
        .render().map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
fn row(title: impl Into<String>, description: impl Into<String>, href: impl Into<String>)
    -> DisplayRow
{
    DisplayRow { title: title.into(), description: description.into(), href: href.into() }
}
fn url_arg(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
fn relation_url(route: &str, id: &RelationId) -> String {
    format!("/catalog/{route}?schema={}&relation={}",
        url_arg(&id.namespace), url_arg(&id.name))
}
fn availability<T: std::fmt::Display>(value: &Availability<T>) -> String {
    match value {
        Availability::Available(n) => n.to_string(),
        Availability::Unavailable => "Unavailable".to_string(),
    }
}
fn validate_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    Ok(())
}
fn params(raw: Option<String>, relation: bool) -> Result<(String, Option<String>), AppError> {
    let raw = raw.ok_or(AppError::new(ErrorKind::InvalidInput))?;
    if raw.len() > 512 { return Err(AppError::new(ErrorKind::InvalidInput)); }
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
fn relation(raw: Option<String>) -> Result<RelationId, AppError> {
    let (namespace, name) = params(raw, true)?;
    Ok(RelationId { namespace, name: name.expect("checked above") })
}
fn visible<T, F>(items: CatalogList<T>, f: F) -> (Vec<DisplayRow>, bool)
where F: Fn(T) -> DisplayRow
{
    (items.items.into_iter().map(f).collect(), items.truncated)
}

pub async fn schemas(State(state): State<AppState>) -> Result<Response, AppError> {
    let Some(c) = state.catalog() else {
        return fragment("Schemas".into(),vec![],false,
            "Not configured. Set CUADB_DATABASE_URL on the server to browse the catalog.");
    };
    let entries = match c.list_schemas().await {
        Ok(entries) => entries,
        Err(db) => {
            return fragment("Schemas unavailable".into(),vec![],false,
                match db.kind {
                    DbErrorKind::PermissionDenied => "Insufficient permissions to read the catalog.",
                    DbErrorKind::TimedOut => "The database catalog did not respond in time.",
                    _ => "The configured database is unavailable.",
                });
        }
    };
    let (rows,truncated) = visible(entries, |n| row(
        n.name.clone(), "Accessible namespace",
        format!("/catalog/relations?schema={}",url_arg(&n.name))
    ));
    fragment("Schemas".into(),rows,truncated,
        "Only schemas with USAGE privileges are listed; empty schemas remain visible.")
}
pub async fn relations(State(state): State<AppState>, RawQuery(raw): RawQuery)
    -> Result<Response, AppError>
{
    let (schema, _) = params(raw,false)?;
    let entries = catalog(&state)?.list_relations(&schema).await.map_err(err)?;
    let (rows,truncated) = visible(entries, |r| row(
        r.id.name.clone(), format!("{:?} · {}.{}",r.kind,r.id.namespace,r.id.name),
        relation_url("structure",&r.id),
    ));
    let mut rows = rows;
    rows.insert(0,row("Advanced objects",
        "Read-only routines, sequences, user types and triggers",
        format!("/catalog/advanced?schema={}",url_arg(&schema))));
    fragment(format!("Relations in {schema}"),rows,truncated,
        "Only objects with SELECT privilege appear; partition parents have their own type.")
}
pub async fn structure(State(state): State<AppState>, RawQuery(raw): RawQuery)
    -> Result<Response, AppError>
{
    let id = relation(raw)?;
    let c = catalog(&state)?;
    let (cols, constraints, partitions) = tokio::try_join!(
        c.describe_columns(&id),
        c.list_constraints(&id),
        c.list_partitions(&id),
    ).map_err(err)?;
    let truncated = cols.truncated || constraints.truncated || partitions.truncated;
    let mut rows = vec![
        row("Indexes", "Unique, primary and expression indexes", relation_url("indexes",&id)),
        row("Storage & statistics", "Exact bytes; estimated rows", relation_url("stats",&id)),
    ];
    for col in cols.items {
        let d = col.column;
        rows.push(row(d.name,format!(
            "{} · {}{}{}{}{}",
            d.native_type,
            if d.nullable {"nullable"} else {"NOT NULL"},
            if d.primary_key {" · primary key"} else {""},
            if col.unique_key {" · unique key member"} else {""},
            col.identity.as_deref().map_or("",|_|" · identity"),
            col.generated.as_deref().map_or("",|_|" · generated"),
        ),String::new()));
        if let Some(def) = d.default_expression {
            rows.push(row("Default expression",def,String::new()));
        }
        if let Some(domain) = col.domain {
            rows.push(row("Domain",domain,String::new()));
        }
        if let Availability::Available(labels) = col.enum_values {
            rows.push(row("Enum labels",labels.join(", "),String::new()));
        }
    }
    for constraint in constraints.items {
        let target = constraint.references.map(|r| format!(" → {}.{}",r.namespace,r.name))
            .unwrap_or_default();
        rows.push(row(constraint.name,
            format!("{:?} ({}){} · {}",
                constraint.kind, constraint.columns.join(", "), target, constraint.definition),
            String::new()));
    }
    for partition in partitions.items {
        rows.push(row(format!("Partition: {}.{}",partition.child.namespace,partition.child.name),
            format!("Parent {}.{} · {}",
                partition.parent.namespace,partition.parent.name,
                partition.bound.unwrap_or_else(|| "No bound available".into())),
            String::new()));
    }
    fragment(format!("Structure · {}.{}",id.namespace,id.name),rows,truncated,
        "Definitions are metadata only; permissions may hide related objects.")
}
pub async fn indexes(State(state): State<AppState>, RawQuery(raw): RawQuery)
    -> Result<Response, AppError>
{
    let id = relation(raw)?;
    let entries = catalog(&state)?.list_indexes(&id).await.map_err(err)?;
    let (rows,truncated) = visible(entries, |i| row(i.name,
        format!("{} · {} · valid={} · {}",
            if i.primary {"Primary"} else if i.unique {"Unique"} else {"Index"},
            i.columns_or_expressions.join(", "),i.valid,i.definition),String::new()));
    fragment(format!("Indexes · {}.{}",id.namespace,id.name),rows,truncated,
        "Expression and partial index definitions are displayed but never executed.")
}
pub async fn stats(State(state): State<AppState>, RawQuery(raw): RawQuery)
    -> Result<Response, AppError>
{
    let id = relation(raw)?;
    let s = catalog(&state)?.relation_statistics(&id).await.map_err(err)?;
    fragment(format!("Storage · {}.{}",id.namespace,id.name),vec![
        row("Estimated rows (approximate)",availability(&s.estimated_rows),""),
        row("Table bytes (exact)",availability(&s.table_bytes),""),
        row("Index bytes (exact)",availability(&s.index_bytes),""),
        row("Total bytes (exact)",availability(&s.total_bytes),""),
    ],false,"Views, foreign tables and partitioned parents can have unavailable metrics.")
}
pub async fn advanced(State(state): State<AppState>, RawQuery(raw): RawQuery)
    -> Result<Response, AppError>
{
    let (schema, _) = params(raw,false)?;
    let entries = catalog(&state)?.list_advanced(&schema).await.map_err(err)?;
    let (rows,truncated) = visible(entries,|o| row(
        format!("{:?} · {}",o.kind,o.name),
        match o.definition {
            Availability::Available(d) => d,
            Availability::Unavailable => "Definition unavailable".into(),
        },String::new()));
    fragment(format!("Advanced objects · {schema}"),rows,truncated,
        "Functions, procedures, sequences, types and triggers are metadata only; never executed.")
}
pub async fn overview(State(state): State<AppState>) -> Result<Response, AppError> {
    let o = catalog(&state)?.advanced_overview().await.map_err(err)?;
    let mut rows = vec![
        row("Visible schemas",o.visible_schema_count.to_string(),""),
        row("Visible relations",o.visible_relation_count.to_string(),""),
    ];
    rows.extend(o.extensions.into_iter().map(|e| row(format!("Extension · {e}"),"Installed","")));
    rows.extend(o.schema_sizes.items.into_iter().map(|s| row(
        format!("Schema · {}",s.namespace),
        format!("Estimated bytes: {}",availability(&s.estimated_bytes)),""
    )));
    rows.extend(o.largest_relations.items.into_iter().map(|(r,size)| row(
        format!("{}.{}",r.namespace,r.name),format!("Estimated bytes: {}",availability(&size)),
        relation_url("structure",&r),
    )));
    fragment("Advanced overview".into(),rows,o.truncated,
        "Overview size estimates use catalog relpages. Counts and charts can be truncated.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_and_duplicate_navigation_parameters() {
        for q in ["schema=", "schema=public&schema=private", "schema=public&host=evil",
                  "schema=public&relation=t&relation=t", "schema=%00"] {
            assert!(params(Some(q.into()),false).is_err());
        }
        assert!(params(Some("schema=Mixed+Case".into()),false).is_ok());
        assert!(relation(Some("schema=public&relation=Mixed%20Name".into())).is_ok());
    }
    #[test]
    fn catalog_content_is_html_escaped() {
        let html = CatalogTemplate {
            title: "Test".into(),
            rows: vec![row("<img src=x onerror=alert(1)>", "value <script>", "")],
            truncated: false,
            note: "",
        }.render().unwrap();
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img src=x"));
        assert!(html.contains("&lt;script&gt;"));
    }
    #[test]
    fn encodes_identifier_links_without_sql_or_secrets() {
        let id = RelationId { namespace: "A & B".into(), name:"x/y?".into() };
        assert_eq!(relation_url("structure",&id),
            "/catalog/structure?schema=A%20%26%20B&relation=x%2Fy%3F");
    }
}
