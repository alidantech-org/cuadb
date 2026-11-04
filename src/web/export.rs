//! Bounded, read-only CSV/JSON export of one relation.
//!
//! Rows come from the same validated adapter paging used by the grid, so
//! identifiers, SELECT privileges, sorting and filters are enforced identically.
//! Independent row, byte and page caps apply; hitting one marks the export
//! truncated (`x-cuadb-export-truncated: true`; JSON also carries the flag).
//! Dropping the request future (request timeout / client disconnect) cancels
//! the loop at the next await point. Linked-table ZIP export is not implemented.

use axum::{
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};

use super::rows::{DataQuery, detail_value, parse};
use crate::{
    db::{
        exports::{MAX_EXPORT_BYTES, MAX_EXPORT_ROWS},
        types::{Cell, RowOptions, RowPage},
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Csv,
    Json,
}

fn invalid() -> AppError {
    AppError::new(ErrorKind::InvalidInput)
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

pub async fn export(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AppError> {
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
        column: query.column,
    })?;
    let catalog = state
        .catalog()
        .ok_or(AppError::new(ErrorKind::DatabaseUnavailable))?;

    let mut columns: Vec<String> = vec![];
    let mut rows: Vec<Vec<Cell>> = vec![];
    let mut truncated = false;
    let mut options: RowOptions = nav.options.clone();
    loop {
        let page: RowPage = catalog
            .fetch_rows(&nav.id, options.clone())
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

    let (body, content_type, ext) = match format {
        Format::Csv => {
            let mut out = String::new();
            let header_cells: Vec<String> = columns
                .iter()
                .map(|c| csv_field(&Cell::Text(c.clone())))
                .collect();
            out.push_str(&header_cells.join(","));
            out.push_str("\r\n");
            for row in &rows {
                let line: Vec<String> = row.iter().map(csv_field).collect();
                out.push_str(&line.join(","));
                out.push_str("\r\n");
            }
            (out, "text/csv; charset=utf-8", "csv")
        }
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
