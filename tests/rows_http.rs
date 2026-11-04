//! Data routes require no PostgreSQL to reject malformed requests.
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
async fn valid_data_route_needs_a_configured_database() {
    let (status, body) = response("/catalog/data?schema=public&relation=orders").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!body.contains("postgresql://"));
}
#[tokio::test]
async fn rejects_browser_sql_and_invalid_navigation() {
    for path in [
        "/catalog/data?schema=public&relation=orders&size=101",
        "/catalog/data?schema=public&relation=orders&page=101",
        "/catalog/data?schema=public&relation=orders&sort=id",
        "/catalog/data?schema=public&relation=orders&sort=id&direction=sideways",
        "/catalog/data?schema=public&relation=orders&cursor=forged",
        "/catalog/data?schema=public&relation=orders&relation=again",
        "/catalog/data?schema=public&relation=orders&sql=delete",
        "/catalog/data?schema=public&relation=orders&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id&column=id",
    ] {
        let (status, _) = response(path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
}
