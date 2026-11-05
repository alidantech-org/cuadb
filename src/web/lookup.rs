//! Search-as-you-type helpers for the row editor: foreign-key lookups (find the referenced
//! row by key or by a label column) and value suggestions from the same column.
//! Everything is bounded, read-only and authorized as a view of the target relation.

use axum::{
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};

use super::rows::{detail_value, name, visible_text};
use crate::{
    auth::{Action, Resource},
    db::types::{RelationId, RowOptions},
    error::{AppError, ErrorKind},
    state::{AppState, checked_profile},
};

#[derive(serde::Deserialize)]
pub struct LookupQuery {
    schema: String,
    relation: String,
    column: String,
    #[serde(default)]
    q: String,
    /// `fk` (rows of the referenced relation) or `values` (distinct values of one column).
    #[serde(default)]
    mode: String,
    /// `1`: report whether exactly this key value exists.
    #[serde(default)]
    exact: String,
    #[serde(default)]
    profile: String,
}

fn texty(native: &str) -> bool {
    let n = native.to_ascii_lowercase();
    n.contains("char") || n.contains("text") || n.contains("name") || n == "citext"
}

/// GET /data/lookup
pub async fn lookup(
    State(state): State<AppState>,
    auth: super::auth::Auth,
    Query(query): Query<LookupQuery>,
) -> Result<Response, AppError> {
    let schema = name(&query.schema)?;
    let relation = name(&query.relation)?;
    let column = name(&query.column)?;
    if query.q.len() > 128 || query.q.chars().any(char::is_control) {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    let fk = match query.mode.as_str() {
        "fk" => true,
        "values" => false,
        _ => return Err(AppError::new(ErrorKind::InvalidInput)),
    };
    let exact = query.exact == "1";
    let profile = checked_profile((!query.profile.is_empty()).then(|| query.profile.clone()))?;
    let pid = state.effective_profile(profile.as_deref());
    auth.authorize(
        &state,
        Action::View,
        &Resource::relation(&pid, &schema, &relation),
    )
    .await?;
    let catalog = state.adapter(profile.as_deref())?;
    let id = RelationId {
        namespace: schema,
        name: relation,
    };
    let details = catalog
        .describe_columns(&id)
        .await
        .map_err(|_| AppError::new(ErrorKind::InvalidInput))?
        .items;
    if !details.iter().any(|d| d.column.name == column) {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    let mut selected = vec![column.clone()];
    if fk {
        selected.extend(
            details
                .iter()
                .filter(|d| d.column.name != column && texty(&d.column.native_type))
                .take(2)
                .map(|d| d.column.name.clone()),
        );
    }
    let limit = if exact {
        50
    } else if fk {
        8
    } else {
        40
    };
    let options = RowOptions {
        limit,
        page: 0,
        selected_columns: selected,
        column_selection_explicit: true,
        sort: None,
        cursor: None,
        filter: (!query.q.is_empty()).then(|| query.q.clone()),
        spatial_geojson: false,
    };
    let page = catalog
        .fetch_rows(&id, options)
        .await
        .map_err(|_| AppError::new(ErrorKind::InvalidInput))?;
    let text = |cell: &crate::db::types::Cell| match cell {
        crate::db::types::Cell::Null => None,
        other => Some(visible_text(&detail_value(other, ""))),
    };
    let body = if fk {
        let items: Vec<_> = page
            .rows
            .iter()
            .filter_map(|row| {
                let value = text(row.first()?)?;
                let label = row
                    .iter()
                    .skip(1)
                    .filter_map(text)
                    .collect::<Vec<_>>()
                    .join(" · ");
                Some(serde_json::json!({ "value": value, "label": label }))
            })
            .collect();
        let found = exact
            && items
                .iter()
                .any(|item| item["value"].as_str() == Some(query.q.as_str()));
        let label = items
            .iter()
            .find(|item| item["value"].as_str() == Some(query.q.as_str()))
            .and_then(|item| item["label"].as_str())
            .unwrap_or_default()
            .to_string();
        serde_json::json!({ "items": items, "found": found, "label": label })
    } else {
        let mut seen = std::collections::BTreeSet::new();
        let items: Vec<String> = page
            .rows
            .iter()
            .filter_map(|row| text(row.first()?))
            .filter(|value| !value.is_empty() && value.len() <= 200 && seen.insert(value.clone()))
            .take(10)
            .collect();
        serde_json::json!({ "items": items })
    };
    let mut response = axum::Json(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
