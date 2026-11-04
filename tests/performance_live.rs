//! Large-table (1,000,000 row) latency and timeout checks. Engines whose
//! CUADB_TEST_*_URL is unset are skipped (NOT evidence). SQLite builds its own file.
//! Run with `-- --nocapture` to see the timing table recorded in docs/performance.md.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{
    app::router_with_state,
    config::Config,
    db::{mysql::MySqlManager, postgres::PgManager, sqlite::SqliteManager},
    state::AppState,
};
use sqlx::Connection;
use tower::ServiceExt;

struct Subject {
    label: &'static str,
    schema: String,
    router: Router,
    timeout_router: Router,
}

fn limits(query_ms: u64) -> cuadb::config::Limits {
    let ms = query_ms.to_string();
    Config::from_values(&[
        ("CUADB_QUERY_TIMEOUT_MS", ms.as_str()),
        ("CUADB_REQUEST_TIMEOUT_MS", "30000"),
    ])
    .unwrap()
    .limits
}

async fn subjects() -> Vec<Subject> {
    let mut out = vec![];
    let mk = |label: &'static str,
              schema: String,
              make: &dyn Fn(
        &cuadb::config::Limits,
    ) -> Arc<dyn cuadb::db::adapter::DatabaseAdapter>| {
        let normal = limits(15_000);
        let tight = limits(100);
        Subject {
            label,
            schema,
            router: router_with_state(&normal, AppState::with_adapter(make(&normal))),
            timeout_router: router_with_state(&tight, AppState::with_adapter(make(&tight))),
        }
    };
    if let Ok(url) = std::env::var("CUADB_TEST_CATALOG_URL") {
        out.push(mk("postgresql", "cuadb_fixture".into(), &|l| {
            Arc::new(PgManager::new(&url, l).unwrap().catalog())
        }));
    }
    if let Ok(url) = std::env::var("CUADB_TEST_MARIADB_URL") {
        out.push(mk("mariadb", "cuadb_fixture_mariadb".into(), &|l| {
            Arc::new(MySqlManager::new(&url, l).unwrap().catalog())
        }));
    }
    if let Ok(url) = std::env::var("CUADB_TEST_MYSQL_URL") {
        out.push(mk("mysql", "cuadb_fixture_mysql".into(), &|l| {
            Arc::new(MySqlManager::new(&url, l).unwrap().catalog())
        }));
    }
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("perf-sqlite");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("perf.db");
    if !file.exists() {
        let mut c =
            sqlx::SqliteConnection::connect(&format!("sqlite://{}?mode=rwc", file.display()))
                .await
                .unwrap();
        for sql in [
            "CREATE TABLE perf_big(id INTEGER PRIMARY KEY, grp INTEGER NOT NULL, payload TEXT NOT NULL, created_at TEXT NOT NULL)",
            "WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM s WHERE n < 1000000) INSERT INTO perf_big SELECT n, n % 1000, hex(randomblob(16)), datetime(1767225600 + n, 'unixepoch') FROM s",
        ] {
            sqlx::query(sql).execute(&mut c).await.unwrap();
        }
        c.close().await.unwrap();
    }
    let url = format!(
        "sqlite:///{}",
        file.display().to_string().trim_start_matches('/')
    );
    out.push(mk("sqlite", "main".into(), &|l| {
        Arc::new(SqliteManager::new(&url, &dir, l).unwrap().catalog())
    }));
    out
}

async fn timed(router: &Router, path: &str) -> (StatusCode, usize, Duration) {
    let start = Instant::now();
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 24).await.unwrap();
    (status, body.len(), start.elapsed())
}

#[tokio::test]
async fn one_million_row_tables_page_sort_and_export_within_budgets() {
    for s in subjects().await {
        let base = format!("/catalog/data?schema={}&relation=perf_big", s.schema);
        let cases = [
            ("first page (25)", format!("{base}&size=25")),
            (
                "page 100 x 100 (offset 10000)",
                format!("{base}&size=100&page=100"),
            ),
            (
                "sort id desc (indexed)",
                format!("{base}&size=25&sort=id&direction=desc"),
            ),
            (
                "sort payload asc (unindexed)",
                format!("{base}&size=25&sort=payload&direction=asc"),
            ),
            (
                "filter literal (full scan)",
                format!("{base}&size=25&filter=zz-not-present-zz"),
            ),
            (
                "structure",
                format!("/catalog/structure?schema={}&relation=perf_big", s.schema),
            ),
            (
                "indexes+stats",
                format!("/catalog/indexes?schema={}&relation=perf_big", s.schema),
            ),
            (
                "export page (100)",
                format!(
                    "/catalog/export?schema={}&relation=perf_big&format=csv&size=100",
                    s.schema
                ),
            ),
            (
                "export relation (10k cap)",
                format!(
                    "/catalog/export?schema={}&relation=perf_big&format=csv&scope=relation",
                    s.schema
                ),
            ),
        ];
        for (name, path) in cases {
            let (status, bytes, took) = timed(&s.router, &path).await;
            eprintln!(
                "PERF {:<11} {:<32} {} {:>8} bytes {:>7.0} ms",
                s.label,
                name,
                status.as_u16(),
                bytes,
                took.as_secs_f64() * 1000.0
            );
            assert_eq!(status, StatusCode::OK, "{} {name}", s.label);
            let cap = if name.starts_with("export relation") {
                4_000_000
            } else {
                400_000
            };
            assert!(
                bytes < cap,
                "{} {name}: response of {bytes} bytes is unbounded",
                s.label
            );
            assert!(
                took < Duration::from_secs(20),
                "{} {name}: {took:?}",
                s.label
            );
        }
        // Constant-size pages: deepest page costs the same bytes as the first page (within 3x).
        let (_, first, _) = timed(&s.router, &format!("{base}&size=100")).await;
        let (_, deep, _) = timed(&s.router, &format!("{base}&size=100&page=100")).await;
        assert!(
            deep < first * 3 && first < deep * 3,
            "{}: page sizes diverge",
            s.label
        );
    }
}

#[tokio::test]
async fn slow_queries_hit_the_query_timeout_quickly_and_the_pool_recovers() {
    for s in subjects().await {
        let slow = format!(
            "/catalog/data?schema={}&relation=perf_big&size=25&sort=payload&direction=desc&filter=ffffffffffffffffffffffffffffff",
            s.schema
        );
        let (status, _, took) = timed(&s.timeout_router, &slow).await;
        eprintln!(
            "PERF {:<11} {:<32} {} {:>7.0} ms (100 ms query timeout)",
            s.label,
            "timeout",
            status.as_u16(),
            took.as_secs_f64() * 1000.0
        );
        // The timeout must fire: 504 (timed out) rather than a full scan result, and quickly.
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT, "{}", s.label);
        assert!(took < Duration::from_secs(3), "{}: took {took:?}", s.label);
        // Same router, same pool: a cheap request still works right after a timeout.
        let (status, _, _) = timed(
            &s.timeout_router,
            &format!("/catalog/data?schema={}&relation=perf_big&size=5", s.schema),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{}: pool must recover", s.label);
    }
}
