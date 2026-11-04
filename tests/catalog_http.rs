//! The read-only browser boundary never needs PostgreSQL to reject
//! invalid input or show a disconnected catalog state.
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::app;
use tower::ServiceExt;

async fn response(path: &str) -> (StatusCode, String) {
    let response = app::router()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 16_384).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn disconnected_catalog_is_helpful_without_database() {
    let (status, html) = response("/catalog/schemas").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Not configured."));
    assert!(html.contains("Database schemas"));
    assert!(!html.contains("postgresql://"));

    let (status, _) = response("/catalog/overview").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn rejects_duplicate_unknown_and_unbounded_identifier_params() {
    for path in [
        "/catalog/relations?schema=public&schema=private",
        "/catalog/relations?schema=public&host=example.com",
        "/catalog/relations?schema=",
        "/catalog/structure?schema=public",
        "/catalog/structure?schema=public&relation=x&relation=y",
        "/catalog/structure?schema=%00&relation=x",
        "/catalog/indexes?schema=public&relation=x&sort=bad",
        "/catalog/stats?schema=public&relation=x&connection=other",
    ] {
        let (status, html) = response(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert!(!html.contains("example.com"));
    }
}

#[tokio::test]
async fn cannot_submit_sql_or_browser_supplied_connection_targets() {
    for path in [
        "/catalog/relations?schema=public&sql=DROP%20TABLE%20users",
        "/catalog/relations?schema=public&url=postgresql%3A%2F%2Ffoo",
        "/catalog/structure?schema=public&relation=users&execute=1",
    ] {
        assert_eq!(response(path).await.0, StatusCode::BAD_REQUEST);
    }
}
