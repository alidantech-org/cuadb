//! Smoke tests that work before the database integration exists.

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let response = cuadb::app::router()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
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
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["x-frame-options"], "DENY");
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("default-src 'self'")
    );
    let body = to_bytes(response.into_body(), 256_000).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("CuaDB"));
    assert!(html.contains("Search tables"));
    assert!(html.contains("id=\"side\""));
    assert!(html.contains("id=\"mainbody\""));
    assert!(!html.contains("placeholder · no database queries"));
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
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("App shell ready")
    );
}

#[tokio::test]
async fn repeated_requests_receive_distinct_trace_ids() {
    let app = cuadb::app::router();
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let second = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let first_id = first.headers().get("x-request-id").unwrap();
    let second_id = second.headers().get("x-request-id").unwrap();
    assert_ne!(first_id, second_id);
}
