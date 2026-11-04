//! HTML endpoints: full pages and reusable fragments rendered by Askama.

use askama::Template;
use axum::response::Html;

use crate::error::AppError;

#[derive(Template)]
#[template(path = "pages/explorer.html")]
struct ExplorerTemplate {
    title: &'static str,
}

#[derive(Template)]
#[template(path = "components/status.html")]
struct StatusTemplate {
    heading: &'static str,
    detail: &'static str,
}

/// Template failures are classified but never exposed to the browser as raw
/// rendering errors, which may contain database-sourced values in future.
fn html<T: Template>(view: T) -> Result<Html<String>, AppError> {
    view.render().map(Html).map_err(|_| AppError::template())
}

pub async fn explorer() -> Result<Html<String>, AppError> {
    html(ExplorerTemplate {
        title: "CuaDB · Database explorer",
    })
}

/// Small, independently requested fragment proving HTMX wiring works.
pub async fn status() -> Result<Html<String>, AppError> {
    html(StatusTemplate {
        heading: "App shell ready",
        detail: "The server is responding. Database connections are not implemented yet.",
    })
}

/// Liveness only: never wait for the database or issue a SQL query here.
pub async fn health() -> &'static str {
    "ok"
}
