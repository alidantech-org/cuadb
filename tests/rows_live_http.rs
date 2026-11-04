//! Live grid routes against a disposable cuadb_fixture_* PostgreSQL database.
//! Returns early (NOT live evidence) when CUADB_TEST_ROWS_URL is unset.

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{app::router_with_state, config::Config, db::postgres::PgManager, state::AppState};
use tower::ServiceExt;

fn url() -> Option<String> {
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

fn app(url: &str) -> (Router, PgManager) {
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(url, &config.limits).unwrap();
    let router = router_with_state(
        &config.limits,
        AppState::with_adapter(Arc::new(manager.catalog())),
    );
    (router, manager)
}

async fn fetch(router: &Router, path: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 24).await.unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

/// href of the footer link with this label, HTML-entity decoded.
fn link(html: &str, label: &str) -> Option<String> {
    let end = html.find(&format!(">{label}</a>"))?;
    let start = html[..end].rfind("href=\"")? + 6;
    let stop = start + html[start..].find('"')?;
    Some(
        html[start..stop]
            .replace("&amp;", "&")
            .replace("&#38;", "&"),
    )
}

const BASE: &str = "/catalog/data?schema=cuadb_fixture&relation=paging&size=50";

#[tokio::test]
async fn live_first_middle_last_pages_have_accurate_controls_and_ranges() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (status, first) = fetch(&router, BASE).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first.contains("<button type=\"button\" disabled>Previous</button>"));
    assert!(first.contains("1–50"));
    let next = link(&first, "Next").expect("next link on first page");
    let (_, second) = fetch(&router, &next).await;
    assert!(second.contains("51–100"));
    assert!(link(&second, "Previous").is_some() && link(&second, "Next").is_some());
    let next = link(&second, "Next").unwrap();
    let (_, last) = fetch(&router, &next).await;
    assert!(last.contains("101–120"));
    assert!(last.contains("<button type=\"button\" disabled>Next</button>"));
    let previous = link(&last, "Previous").unwrap();
    let (_, back) = fetch(&router, &previous).await;
    assert!(back.contains("51–100"));
    let (_, empty) = fetch(
        &router,
        "/catalog/data?schema=cuadb_fixture&relation=empty_paging",
    )
    .await;
    assert!(empty.contains("0–0") && empty.contains("disabled>Next<"));
    manager.close().await;
}

#[tokio::test]
async fn live_size_and_page_bounds_are_rejected_before_the_database() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    for bad in [
        "&size=0",
        "&size=101",
        "&size=4294967295",
        "&page=101",
        "&page=-1",
        "&cursor=zz",
        "&sort=id",
        "&direction=asc",
        "&sort=nope&direction=asc",
        "&sort=id&direction=sideways",
    ] {
        let (status, _) = fetch(&router, &format!("{BASE}{bad}")).await;
        assert!(status.is_client_error(), "{bad} -> {status}");
    }
    manager.close().await;
}

#[tokio::test]
async fn live_sorting_nulls_are_deterministic_and_sorted_export_is_complete() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let export = |dir: &'static str| {
        format!(
            "/catalog/export?schema=cuadb_fixture&relation=paging&format=json&scope=relation&sort=nullable_text&direction={dir}"
        )
    };
    let (_, asc) = fetch(&router, &export("asc")).await;
    let (_, again) = fetch(&router, &export("asc")).await;
    assert_eq!(asc, again, "sorted order must be repeatable");
    let doc: serde_json::Value = serde_json::from_str(&asc).unwrap();
    let rows = doc["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 120);
    let mut ids: Vec<i64> = rows.iter().map(|r| r[0].as_i64().unwrap()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        120,
        "no duplicate or missing rows across sorted pages"
    );
    // CuaDB always orders NULLs last (explicit NULLS LAST), both directions.
    let nulls = |doc: &serde_json::Value| -> Vec<bool> {
        doc["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r[2].is_null())
            .collect()
    };
    let asc_nulls = nulls(&doc);
    assert!(asc_nulls[..96].iter().all(|n| !n) && asc_nulls[96..].iter().all(|n| *n));
    let (_, desc) = fetch(&router, &export("desc")).await;
    let desc_doc: serde_json::Value = serde_json::from_str(&desc).unwrap();
    let desc_nulls = nulls(&desc_doc);
    assert!(desc_nulls[..96].iter().all(|n| !n) && desc_nulls[96..].iter().all(|n| *n));
    manager.close().await;
}

#[tokio::test]
async fn live_rapid_repeated_sort_requests_never_mix_states() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let mut tasks = vec![];
    for i in 0..16 {
        let router = router.clone();
        tasks.push(tokio::spawn(async move {
            let dir = if i % 2 == 0 { "asc" } else { "desc" };
            let (status, body) =
                fetch(&router, &format!("{BASE}&sort=amount&direction={dir}")).await;
            (i, dir, status, body)
        }));
    }
    for task in tasks {
        let (_, dir, status, body) = task.await.unwrap();
        assert_eq!(status, StatusCode::OK);
        let expected = if dir == "asc" {
            "ascending"
        } else {
            "descending"
        };
        assert!(body.contains(&format!("aria-sort=\"{expected}\"")));
        assert!(body.contains("1–50"));
    }
    manager.close().await;
}

#[tokio::test]
async fn live_large_values_are_truncated_and_markup_is_escaped() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (_, body) = fetch(&router, &format!("{BASE}&sort=id&direction=asc")).await;
    // Row 1 stores 1000 characters; the page must not ship them all.
    assert!(!body.contains(&"Ω".repeat(500)));
    assert!(!body.contains("<script"));
    manager.close().await;
}

#[tokio::test]
async fn live_navigation_links_keep_sort_and_size_and_sorting_resets_paging() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (_, first) = fetch(
        &router,
        &format!("{BASE}&sort=amount&direction=desc&column=id&column=amount&pick=1"),
    )
    .await;
    let next = link(&first, "Next")
        .unwrap_or_else(|| panic!("no next: {}", &first[..first.len().min(600)]));
    for kept in [
        "size=50",
        "sort=amount",
        "direction=desc",
        "column=id",
        "column=amount",
        "pick=1",
    ] {
        assert!(next.contains(kept), "{kept} lost in {next}");
    }
    let (_, second) = fetch(&router, &next).await;
    assert!(second.contains("51–100"));
    // Header sort links on a later page restart at page 1: no cursor, no page.
    let header_link = link(&second, "▲").or_else(|| {
        let start = second.find("sort=amount")?;
        let from = second[..start].rfind("href=\"")? + 6;
        let to = from + second[from..].find('"')?;
        Some(second[from..to].replace("&#38;", "&"))
    });
    let header_link = header_link.expect("a sort link exists in the header");
    assert!(!header_link.contains("cursor=") && !header_link.contains("page="));
    manager.close().await;
}
