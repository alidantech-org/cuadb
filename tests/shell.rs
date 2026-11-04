//! Smoke tests that work before the database integration exists.

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let response = cuadb::app::router()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(body.as_ref(), b"ok");
}

#[tokio::test]
async fn home_renders_an_html_shell() {
    let response = cuadb::app::router()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 256_000).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("CuaDB"));
    assert!(html.contains("Data preview"));
}

#[tokio::test]
async fn status_fragment_returns_html() {
    let response = cuadb::app::router()
        .oneshot(
            Request::builder()
                .uri("/fragments/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64_000).await.unwrap();
    assert!(String::from_utf8(body.to_vec())
        .unwrap()
        .contains("App shell ready"));
}
