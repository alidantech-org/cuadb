//! Live structure, storage and rich-value routes against a disposable
//! cuadb_fixture_* PostgreSQL database (PostGIS enabled). Tests return early
//! (NOT live evidence) when CUADB_TEST_CATALOG_URL is unset.

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{app::router_with_state, config::Config, db::postgres::PgManager, state::AppState};
use tower::ServiceExt;

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

fn text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ')
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&#39;", "'")
        .replace("&#34;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[tokio::test]
async fn live_structure_shows_composite_keys_defaults_generation_and_fk_targets() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (status, html) = fetch(
        &router,
        "/catalog/structure?schema=cuadb_fixture&relation=orders",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let t = text(&html);
    assert!(
        t.contains("1 ID integer no Primary key — Identity ALWAYS"),
        "{t}"
    );
    assert!(t.contains("2 Code character varying(20) no Primary key"));
    assert!(t.contains("Generated STORED"));
    assert!(t.contains("enum (new, approved)") && t.contains("domain cuadb_fixture.nonnegative"));
    assert!(t.contains("orders_account_id_fkey ForeignKey account_id cuadb_fixture.accounts (id)"));
    assert!(t.contains("orders_pkey Primary ID, Code"));
    // Tab navigation marks Structure as the current view.
    assert!(html.contains("aria-current=\"page\">Structure</a>"));
    manager.close().await;
}

#[tokio::test]
async fn live_views_partition_parents_and_missing_relations_are_handled() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (_, view) = fetch(
        &router,
        "/catalog/structure?schema=cuadb_fixture&relation=accounts_view",
    )
    .await;
    assert!(text(&view).contains("No visible constraints"));
    let (_, parent) = fetch(
        &router,
        "/catalog/structure?schema=cuadb_fixture&relation=events",
    )
    .await;
    let parent = text(&parent);
    assert!(parent.contains("cuadb_fixture.events_2026 FOR VALUES FROM"));
    let (status, _) = fetch(
        &router,
        "/catalog/structure?schema=cuadb_fixture&relation=not_there",
    )
    .await;
    assert!(status.is_client_error());
    let (status, _) = fetch(
        &router,
        "/catalog/structure?schema=pg_catalog&relation=pg_authid",
    )
    .await;
    assert!(
        status.is_client_error(),
        "inaccessible catalog relation must not leak"
    );
    manager.close().await;
}

#[tokio::test]
async fn live_storage_stats_are_estimates_with_unavailable_partition_values_and_no_count_scan() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (_, orders) = fetch(
        &router,
        "/catalog/stats?schema=cuadb_fixture&relation=orders",
    )
    .await;
    let orders = text(&orders);
    assert!(orders.contains("Estimated rows 2 Engine-reported estimate; not an exact COUNT(*)"));
    assert!(orders.contains("Table data size 8192"));
    let (_, parent) = fetch(
        &router,
        "/catalog/stats?schema=cuadb_fixture&relation=events",
    )
    .await;
    let parent = text(&parent);
    assert!(parent.contains("PartitionedTable") && parent.contains("Estimated rows Unavailable"));
    // The refresh route is the same bounded catalog read; repeated calls are stable.
    let (_, again) = fetch(
        &router,
        "/catalog/stats?schema=cuadb_fixture&relation=orders",
    )
    .await;
    assert_eq!(text(&again), orders);
    let (_, empty) = fetch(
        &router,
        "/catalog/stats?schema=cuadb_fixture&relation=empty_stats_fixture",
    )
    .await;
    assert!(text(&empty).contains("Estimated rows 0"));
    manager.close().await;
}

#[tokio::test]
async fn live_rich_values_are_escaped_bounded_and_keep_exact_semantics() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (status, html) = fetch(
        &router,
        "/catalog/data?schema=cuadb_fixture&relation=rich_values&size=25",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!html.contains("<script>alert(1)"), "markup must be escaped");
    assert!(
        !html.contains('\u{7}') && !html.contains('\u{202E}'),
        "controls must be made visible"
    );
    assert!(html.contains("\\u{0007}") && html.contains("\\u{202E}"));
    assert!(
        html.contains("12345678901234567890.1234567890"),
        "exact numeric text"
    );
    assert!(
        html.contains("{1,2,NULL,4}") && html.contains("approved") && html.contains("\\x00ff10")
    );
    assert!(
        html.contains("123e4567-e89b-12d3-a456-426614174000") && html.contains("192.168.0.1/24")
    );
    assert!(
        html.contains("2026-01-02 00:04:05.123456+00"),
        "timestamptz keeps microseconds and offset"
    );
    // 20k-element JSON/array, 20 KB bytea and a 100 KB text value must not inflate the page.
    assert!(html.len() < 120_000, "page was {} bytes", html.len());
    assert!(html.contains("aria-label=\"truncated\""));
    manager.close().await;
}

#[tokio::test]
async fn live_postgis_geometry_reports_type_srid_and_bounded_geojson() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    let (_, plain) = fetch(
        &router,
        "/catalog/data?schema=cuadb_fixture&relation=places&size=25",
    )
    .await;
    assert!(plain.contains("geometry"));
    assert!(plain.contains("POINT(36.82 -1.29)"));
    assert!(!plain.contains("coordinates"), "GeoJSON is opt-in");
    let (status, geo) = fetch(
        &router,
        "/catalog/data?schema=cuadb_fixture&relation=places&size=25&spatial=geojson",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(geo.contains("coordinates") && geo.contains("36.82") && geo.contains("Point"));
    let (_, structure) = fetch(
        &router,
        "/catalog/structure?schema=cuadb_fixture&relation=places",
    )
    .await;
    assert!(text(&structure).contains("geometry"));
    let (_, overview) = fetch(&router, "/catalog/overview").await;
    let overview = text(&overview);
    assert!(overview.contains("Extensions") && overview.contains("postgis"));
    manager.close().await;
}

#[tokio::test]
async fn live_spatial_edge_cases_never_crash_or_mislead_and_stay_bounded() {
    let Some(url) = url() else { return };
    let (router, manager) = app(&url);
    for query in ["", "&spatial=geojson"] {
        let (status, html) = fetch(
            &router,
            &format!("/catalog/data?schema=cuadb_fixture&relation=spatial_edges&size=25{query}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}");
        // A 10,001-vertex polygon, an empty point, a non-4326 SRID and geography coexist.
        assert!(
            html.len() < 150_000,
            "{query}: page was {} bytes",
            html.len()
        );
        assert!(html.contains("geography") && html.contains("geometry"));
        assert!(html.contains("EMPTY"), "empty geometry keeps its meaning");
        assert!(
            html.contains("POLYGON"),
            "self-intersecting polygon stays readable"
        );
        if query.is_empty() {
            assert!(!html.contains("coordinates"));
        } else {
            // Projected data is labelled with its CRS, never passed off as lon/lat.
            assert!(html.contains("EPSG:3857"));
            assert!(
                html.contains("GeoJSON detail exceeds"),
                "huge geometry is refused, not inlined"
            );
            assert!(
                html.contains("&#34;coordinates&#34;: []"),
                "empty point keeps empty coordinates"
            );
        }
    }
    manager.close().await;
}
