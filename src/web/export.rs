//! Bounded, read-only CSV/JSON export of one relation.
//!
//! Rows come from the same validated adapter paging used by the grid, so
//! identifiers, SELECT privileges, sorting and filters are enforced identically.
//! Independent row, byte and page caps apply; hitting one marks the export
//! truncated (`x-cuadb-export-truncated: true`; JSON also carries the flag).
//! Dropping the request future (request timeout / client disconnect) cancels
//! the loop at the next await point. Linked-table ZIP export is not implemented.

use axum::{
    extract::{RawQuery, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};

use super::rows::{DataQuery, detail_value, parse};
use crate::{
    db::{
        adapter::DatabaseAdapter,
        exports::{
            ExportFormat, ExportPlan, ExportScope, MAX_EXPORT_BYTES, MAX_EXPORT_RELATIONS,
            MAX_EXPORT_ROWS,
        },
        types::{Cell, RelationId, RowOptions, RowPage},
    },
    error::{AppError, ErrorKind},
    state::AppState,
};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportQuery {
    schema: String,
    relation: String,
    format: String,
    /// `page` (default): the single requested grid page. `relation`: bounded walk.
    scope: Option<String>,
    size: Option<u32>,
    page: Option<u32>,
    sort: Option<String>,
    direction: Option<String>,
    filter: Option<String>,
    #[serde(default)]
    column: Vec<String>,
    /// Related-table ZIP only: foreign-key hops (0–2, default 1).
    depth: Option<u8>,
    /// Trusted connection profile id; absent means the default profile.
    profile: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Csv,
    Json,
}

fn invalid() -> AppError {
    AppError::new(ErrorKind::InvalidInput)
}

impl ExportQuery {
    fn from_raw(raw: Option<&str>) -> Result<Self, AppError> {
        let (mut map, column) = super::rows::split_query(
            raw,
            &[
                "schema",
                "relation",
                "format",
                "scope",
                "size",
                "page",
                "sort",
                "direction",
                "filter",
                "column",
                "depth",
                "profile",
            ],
        )?;
        Ok(Self {
            schema: map.remove("schema").ok_or_else(invalid)?,
            relation: map.remove("relation").ok_or_else(invalid)?,
            format: map.remove("format").ok_or_else(invalid)?,
            scope: map.remove("scope"),
            size: map
                .remove("size")
                .map(|v| v.parse().map_err(|_| invalid()))
                .transpose()?,
            page: map
                .remove("page")
                .map(|v| v.parse().map_err(|_| invalid()))
                .transpose()?,
            sort: map.remove("sort"),
            direction: map.remove("direction"),
            filter: map.remove("filter"),
            column,
            profile: map.remove("profile"),
            depth: map
                .remove("depth")
                .map(|v| v.parse().map_err(|_| invalid()))
                .transpose()?,
        })
    }
}

fn cell_text(cell: &Cell) -> String {
    detail_value(cell, "")
}

/// Neutralize spreadsheet formula injection for text-origin cells.
fn csv_field(cell: &Cell) -> String {
    if matches!(cell, Cell::Null) {
        return String::new();
    }
    let mut text = cell_text(cell);
    if matches!(cell, Cell::Text(_) | Cell::Truncated { .. })
        && text.starts_with(['=', '+', '-', '@', '\t', '\r'])
    {
        text.insert(0, '\'');
    }
    if text.contains([',', '"', '\n', '\r']) || text.starts_with('\'') {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

fn json_value(cell: &Cell) -> serde_json::Value {
    match cell {
        Cell::Null => serde_json::Value::Null,
        Cell::Boolean(v) => (*v).into(),
        Cell::Integer(v) => (*v).into(),
        other => cell_text(other).into(),
    }
}

fn safe_filename(schema: &str, relation: &str, ext: &str) -> String {
    let clean: String = format!("{schema}_{relation}")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{clean}.{ext}")
}

fn has_truncated_cells(rows: &[Vec<Cell>]) -> bool {
    rows.iter()
        .flatten()
        .any(|c| matches!(c, Cell::Truncated { .. } | Cell::Unsupported { .. }))
}

struct Collected {
    columns: Vec<String>,
    rows: Vec<Vec<Cell>>,
    truncated: bool,
}

/// Page through one relation under the row cap; cancellable at every await.
async fn collect(
    catalog: &dyn DatabaseAdapter,
    id: &RelationId,
    mut options: RowOptions,
    whole: bool,
) -> Result<Collected, AppError> {
    let mut columns: Vec<String> = vec![];
    let mut rows: Vec<Vec<Cell>> = vec![];
    let mut truncated = false;
    loop {
        let page: RowPage = catalog
            .fetch_rows(id, options.clone())
            .await
            .map_err(super::rows::classified)?;
        if columns.is_empty() {
            columns = page.columns.iter().map(|c| c.name.clone()).collect();
        }
        truncated |= page.truncated_columns || has_truncated_cells(&page.rows);
        let room = (MAX_EXPORT_ROWS as usize).saturating_sub(rows.len());
        if page.rows.len() > room {
            rows.extend(page.rows.into_iter().take(room));
            truncated = true;
            break;
        }
        rows.extend(page.rows);
        if !whole || !page.has_more {
            break;
        }
        if rows.len() as u64 >= MAX_EXPORT_ROWS {
            truncated = true;
            break;
        }
        match page.next_cursor {
            Some(cursor) => options.cursor = Some(cursor),
            None if options.page < 100 => options.page += 1,
            None => {
                truncated = true;
                break;
            }
        }
    }
    Ok(Collected {
        columns,
        rows,
        truncated,
    })
}

fn csv_text(columns: &[String], rows: &[Vec<Cell>]) -> String {
    let mut out = String::new();
    let header: Vec<String> = columns
        .iter()
        .map(|c| csv_field(&Cell::Text(c.clone())))
        .collect();
    out.push_str(&header.join(","));
    out.push_str("\r\n");
    for row in rows {
        let line: Vec<String> = row.iter().map(csv_field).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out
}

/// Related-table ZIP: the root relation plus relations it references through
/// foreign keys (outgoing only), breadth-first to `depth`. Every relation is a
/// bounded CSV; the archive has independent relation, row-per-relation and total
/// byte caps. Unreadable linked relations are listed in the manifest, not fatal.
async fn related_zip(
    catalog: &dyn DatabaseAdapter,
    root: RelationId,
    depth: u8,
    may_export: &(dyn Fn(&RelationId) -> bool + Sync),
) -> Result<(Vec<u8>, bool), AppError> {
    use std::io::Write;
    ExportPlan {
        relations: vec![root.clone()],
        format: ExportFormat::RelatedZip,
        scope: ExportScope::RelatedRelations,
        max_rows: MAX_EXPORT_ROWS,
        max_bytes: MAX_EXPORT_BYTES,
        max_depth: depth,
    }
    .bounded()
    .map_err(|_| invalid())?;
    let mut order = vec![root.clone()];
    let mut edges: Vec<serde_json::Value> = vec![];
    let mut frontier = vec![root];
    for _ in 0..depth {
        let mut next = vec![];
        for id in &frontier {
            let Ok(list) = catalog.list_constraints(id).await else {
                continue;
            };
            for constraint in list.items {
                let Some(target) = constraint.references else {
                    continue;
                };
                // Relations the user may not export are neither followed nor revealed.
                if !may_export(&target) {
                    continue;
                }
                edges.push(serde_json::json!({
                    "from": format!("{}.{}", id.namespace, id.name),
                    "to": format!("{}.{}", target.namespace, target.name),
                    "constraint": constraint.name,
                    "columns": constraint.columns,
                    "referenced_columns": constraint.referenced_columns,
                }));
                if !order.contains(&target) && order.len() < MAX_EXPORT_RELATIONS {
                    order.push(target.clone());
                    next.push(target);
                }
            }
        }
        frontier = next;
    }
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut total: u64 = 0;
    let mut truncated = false;
    let mut manifest: Vec<serde_json::Value> = vec![];
    for id in &order {
        let name = format!(
            "{}.csv",
            safe_filename(&id.namespace, &id.name, "csv").trim_end_matches(".csv")
        );
        match collect(
            catalog,
            id,
            RowOptions {
                limit: 100,
                ..RowOptions::default()
            },
            true,
        )
        .await
        {
            Ok(done) => {
                let text = csv_text(&done.columns, &done.rows);
                if total + text.len() as u64 > MAX_EXPORT_BYTES {
                    truncated = true;
                    manifest.push(serde_json::json!({"relation": format!("{}.{}", id.namespace, id.name), "status": "omitted: byte limit"}));
                    continue;
                }
                total += text.len() as u64;
                truncated |= done.truncated;
                writer
                    .start_file(&name, opts)
                    .map_err(|_| AppError::new(ErrorKind::Internal))?;
                writer
                    .write_all(text.as_bytes())
                    .map_err(|_| AppError::new(ErrorKind::Internal))?;
                manifest.push(serde_json::json!({"relation": format!("{}.{}", id.namespace, id.name), "file": name, "rows": done.rows.len(), "truncated": done.truncated}));
            }
            Err(error) if order[0] == *id => return Err(error),
            Err(_) => {
                manifest.push(serde_json::json!({"relation": format!("{}.{}", id.namespace, id.name), "status": "omitted: not readable"}));
            }
        }
    }
    let meta = serde_json::json!({"relations": manifest, "relationships": edges, "truncated": truncated, "depth": depth}).to_string();
    writer
        .start_file("manifest.json", opts)
        .map_err(|_| AppError::new(ErrorKind::Internal))?;
    writer
        .write_all(meta.as_bytes())
        .map_err(|_| AppError::new(ErrorKind::Internal))?;
    let bytes = writer
        .finish()
        .map_err(|_| AppError::new(ErrorKind::Internal))?
        .into_inner();
    Ok((bytes, truncated))
}

pub async fn export(
    State(state): State<AppState>,
    auth: crate::web::auth::Auth,
    RawQuery(raw): RawQuery,
) -> Result<Response, AppError> {
    let query = ExportQuery::from_raw(raw.as_deref())?;
    if query.format == "zip" {
        let plain = query.size.is_none()
            && query.page.is_none()
            && query.sort.is_none()
            && query.direction.is_none()
            && query.filter.is_none()
            && query.column.is_empty();
        if query.scope.as_deref() != Some("related") || !plain {
            return Err(invalid());
        }
        let id = RelationId {
            namespace: super::rows::name(&query.schema)?,
            name: super::rows::name(&query.relation)?,
        };
        let zip_profile = crate::state::checked_profile(query.profile)?;
        let catalog = state.adapter(zip_profile.as_deref())?;
        let pid = state.effective_profile(zip_profile.as_deref());
        auth.authorize(
            &state,
            crate::auth::Action::Export,
            &crate::auth::Resource::relation(&pid, &id.namespace, &id.name),
        )
        .await?;
        let principal = auth.0.clone();
        let may_export = move |target: &RelationId| {
            principal.allows(
                crate::auth::Action::Export,
                &crate::auth::Resource::relation(&pid, &target.namespace, &target.name),
            )
        };
        crate::auth::audit_export(&state, &auth.0, &id, "zip").await;
        let (bytes, truncated) = related_zip(
            catalog.as_ref(),
            id.clone(),
            query.depth.unwrap_or(1),
            &may_export,
        )
        .await?;
        let mut response = bytes.into_response();
        let headers = response.headers_mut();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/zip"),
        );
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"{}\"",
                safe_filename(&id.namespace, &id.name, "zip")
            ))
            .map_err(|_| invalid())?,
        );
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(
            "x-cuadb-export-truncated",
            HeaderValue::from_static(if truncated { "true" } else { "false" }),
        );
        return Ok(response);
    }
    if query.depth.is_some() {
        return Err(invalid());
    }
    let format = match query.format.as_str() {
        "csv" => Format::Csv,
        "json" => Format::Json,
        _ => return Err(invalid()),
    };
    let whole = match query.scope.as_deref() {
        None | Some("page") => false,
        Some("relation") => true,
        Some(_) => return Err(invalid()),
    };
    if whole && query.page.is_some() {
        return Err(invalid());
    }
    let nav = parse(DataQuery {
        schema: query.schema,
        relation: query.relation,
        size: Some(if whole {
            100
        } else {
            query.size.unwrap_or(100)
        }),
        page: query.page,
        sort: query.sort,
        direction: query.direction,
        cursor: None,
        filter: query.filter,
        spatial: None,
        pick: (!query.column.is_empty()).then(|| "1".into()),
        profile: query.profile,
        column: query.column,
    })?;
    let pid = state.effective_profile(nav.profile.as_deref());
    auth.authorize(
        &state,
        crate::auth::Action::Export,
        &crate::auth::Resource::relation(&pid, &nav.id.namespace, &nav.id.name),
    )
    .await?;
    crate::auth::audit_export(&state, &auth.0, &nav.id, &query.format).await;
    let catalog = state.adapter(nav.profile.as_deref())?;

    let Collected {
        columns,
        rows,
        truncated,
    } = collect(catalog.as_ref(), &nav.id, nav.options.clone(), whole).await?;

    let (body, content_type, ext) = match format {
        Format::Csv => (csv_text(&columns, &rows), "text/csv; charset=utf-8", "csv"),
        Format::Json => {
            let doc = serde_json::json!({
                "columns": columns,
                "rows": rows.iter().map(|r| r.iter().map(json_value).collect::<Vec<_>>()).collect::<Vec<_>>(),
                "truncated": truncated,
            });
            (doc.to_string(), "application/json", "json")
        }
    };
    if body.len() as u64 > MAX_EXPORT_BYTES {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            safe_filename(&nav.id.namespace, &nav.id.name, ext)
        ))
        .map_err(|_| invalid())?,
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-cuadb-export-truncated",
        HeaderValue::from_static(if truncated { "true" } else { "false" }),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_neutralize_formulas_and_quote_specials() {
        assert_eq!(csv_field(&Cell::Text("=1+1".into())), "\"'=1+1\"");
        assert_eq!(csv_field(&Cell::Integer(-5)), "-5");
        assert_eq!(csv_field(&Cell::Null), "");
        assert_eq!(csv_field(&Cell::Text("a,\"b\"".into())), "\"a,\"\"b\"\"\"");
        assert_eq!(csv_field(&Cell::Text("plain".into())), "plain");
    }

    #[test]
    fn filenames_are_ascii_safe() {
        assert_eq!(safe_filename("p\"ub", "a/b", "csv"), "p_ub_a_b.csv");
    }
}
