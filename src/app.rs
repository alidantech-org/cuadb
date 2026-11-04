//! Axum route registration. Keep routing independent from database adapters.

use axum::{Router, routing::get};
use tower_http::services::ServeDir;

use crate::web;

/// Construct the router for both the binary and integration tests.
pub fn router() -> Router {
    Router::new()
        .route("/", get(web::explorer))
        .route("/health", get(web::health))
        .route("/fragments/status", get(web::status))
        .nest_service("/static", ServeDir::new("public"))
}
