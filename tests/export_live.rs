//! Live export checks against a disposable cuadb_fixture_* PostgreSQL database.
//! Returns early (NOT counted as live evidence) when CUADB_TEST_ROWS_URL is unset.

use std::{io::Read, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{app::router_with_state, config::Config, db::postgres::PgManager, state::AppState};
use tower::ServiceExt;

async fn fetch(url: &str, path: &str) -> (StatusCode, Vec<u8>) {
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(url, &config.limits).unwrap();
    let app = router_with_state(
        &config.limits,
        AppState::with_adapter(Arc::new(manager.catalog())),
    );
    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 24)
        .await
        .unwrap()
        .to_vec();
    manager.close().await;
    (status, body)
}

fn fixture_url() -> Option<String> {
    let url = std::env::var("CUADB_TEST_ROWS_URL").ok()?;
    assert!(
        url::Url::parse(&url)
            .unwrap()
            .path()
            .trim_start_matches('/')
            .starts_with("cuadb_fixture_")
    );
    Some(url)
}

#[tokio::test]
async fn live_relation_csv_walks_every_row_with_stable_paging() {
    let Some(url) = fixture_url() else { return };
    let (status, body) = fetch(
        &url,
        "/catalog/export?schema=cuadb_fixture&relation=paging&format=csv&scope=relation&sort=id&direction=asc",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let text = String::from_utf8(body).unwrap();
    let lines: Vec<&str> = text.trim_end().split("\r\n").collect();
    assert_eq!(lines.len(), 121, "header + 120 fixture rows");
    assert!(lines[0].starts_with("id,amount"));
    assert!(lines[1].starts_with("1,"));
    assert!(lines[120].starts_with("120,"));
}

#[tokio::test]
async fn live_json_export_reports_complete_untruncated_small_relation() {
    let Some(url) = fixture_url() else { return };
    let (status, body) = fetch(
        &url,
        "/catalog/export?schema=cuadb_fixture&relation=single_paging&format=json",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(doc["rows"].as_array().unwrap().len(), 1);
    assert_eq!(doc["truncated"], false);
}

#[tokio::test]
async fn live_related_zip_includes_foreign_key_targets_and_rejects_hidden_input() {
    let Some(url) = fixture_url() else { return };
    let (status, body) = fetch(
        &url,
        "/catalog/export?schema=cuadb_fixture&relation=orders&format=zip&scope=related",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(body)).unwrap();
    let mut names: Vec<String> = archive.file_names().map(String::from).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "cuadb_fixture_accounts.csv",
            "cuadb_fixture_orders.csv",
            "manifest.json"
        ]
    );
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    assert!(manifest.contains("cuadb_fixture.accounts"));

    let (status, _) = fetch(
        &url,
        "/catalog/export?schema=cuadb_fixture&relation=nope&format=csv",
    )
    .await;
    assert!(status.is_client_error() || status.is_server_error());
}
