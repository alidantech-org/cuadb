//! Live navigation routes against a disposable cuadb_fixture_* PostgreSQL database.
//! Returns early (NOT live evidence) when CUADB_TEST_CATALOG_URL is unset.

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{app::router_with_state, config::Config, db::postgres::PgManager, state::AppState};
use tower::ServiceExt;

async fn fetch(url: &str, path: &str) -> (StatusCode, String) {
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
    let body = to_bytes(response.into_body(), 1 << 22).await.unwrap();
    manager.close().await;
    (status, String::from_utf8_lossy(&body).into_owned())
}

fn url() -> Option<String> {
    let url = std::env::var("CUADB_TEST_CATALOG_URL").ok()?;
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
async fn live_tree_lists_mixed_case_and_unicode_names_and_hides_system_schemas() {
    let Some(url) = url() else { return };
    let (status, schemas) = fetch(&url, "/catalog/schemas").await;
    assert_eq!(status, StatusCode::OK);
    assert!(schemas.contains("cuadb_fixture") && schemas.contains("CuaDB Mixed"));
    assert!(!schemas.contains("pg_catalog") && !schemas.contains("information_schema"));

    let (status, relations) = fetch(&url, "/catalog/relations?schema=CuaDB%20Mixed").await;
    assert_eq!(status, StatusCode::OK);
    assert!(relations.contains("Ünicode Table"));
}

#[tokio::test]
async fn live_relation_search_handles_unicode_case_and_empty_results() {
    let Some(url) = url() else { return };
    let (_, found) = fetch(
        &url,
        "/catalog/relations?schema=CuaDB%20Mixed&q=%C3%BCnicode",
    )
    .await;
    assert!(found.contains("Ünicode Table"));
    let (status, none) = fetch(
        &url,
        "/catalog/relations?schema=cuadb_fixture&q=no-such-table",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!none.contains("hx-get") && none.contains("Filtered by name"));
    let (_, kinds) = fetch(&url, "/catalog/relations?schema=cuadb_fixture").await;
    for expected in ["accounts", "orders", "events", "paging"] {
        assert!(kinds.contains(expected), "{expected}");
    }
}

#[tokio::test]
async fn live_overview_and_metadata_objects_render_without_inline_code() {
    let Some(url) = url() else { return };
    let (status, overview) = fetch(&url, "/catalog/overview").await;
    assert_eq!(status, StatusCode::OK);
    assert!(overview.contains("Size by schema") && overview.contains("Largest tables"));
    assert!(overview.contains("data-pct=") && !overview.contains(" style="));
    let (status, objects) = fetch(&url, "/catalog/objects?schema=cuadb_fixture").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!objects.contains(" style=") && !objects.contains("<script"));
    // Whatever kinds exist link to a detail page that shows inert definition text.
    let link = objects
        .split("hx-get=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .map(|l| l.replace("&#38;", "&").replace("&amp;", "&"));
    if let Some(link) = link {
        let (status, page) = fetch(&url, &link).await;
        assert_eq!(status, StatusCode::OK, "{link}");
        assert!(page.contains("metadata only, never executed") && page.contains("class=\"src\""));
    }
    for bad in [
        "/catalog/object?schema=cuadb_fixture&kind=table&name=x",
        "/catalog/object?schema=cuadb_fixture&kind=function&name=%00",
    ] {
        assert!(fetch(&url, bad).await.0.is_client_error(), "{bad}");
    }
}
