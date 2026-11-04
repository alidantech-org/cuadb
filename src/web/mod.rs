//! HTML endpoints: full pages and reusable fragments rendered by Askama.

use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

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

/// Render consistently; template errors must never return raw template data.
fn html<T: Template>(view: T) -> Response {
    match view.render() {
        Ok(body) => Html(body).into_response(),
        Err(error) => {
            eprintln!("Template rendering failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn explorer() -> Response {
    html(ExplorerTemplate {
        title: "CuaDB · Database explorer",
    })
}

/// Small, independently requested fragment proving HTMX wiring works.
pub async fn status() -> Response {
    html(StatusTemplate {
        heading: "App shell ready",
        detail: "The server is responding. Database connections are not implemented yet.",
    })
}

/// Container and local development health checks need no database credentials.
pub async fn health() -> &'static str {
    "ok"
}
