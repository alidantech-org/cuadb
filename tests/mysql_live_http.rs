//! Live MySQL 8 and MariaDB 11.8 route checks. Each engine is tested independently
//! against its own cuadb_fixture_* database; an engine whose CUADB_TEST_*_URL is
//! unset is skipped (and is NOT live evidence).

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{
    app::router_with_state,
    config::Config,
    db::{mysql::MySqlManager, types::ConnectionStatus},
    state::AppState,
};
use tower::ServiceExt;

struct Engine {
    url: String,
    schema: &'static str,
}

fn engines() -> Vec<Engine> {
    [
        ("CUADB_TEST_MARIADB_URL", "cuadb_fixture_mariadb"),
        ("CUADB_TEST_MYSQL_URL", "cuadb_fixture_mysql"),
    ]
    .into_iter()
    .filter_map(|(env, schema)| {
        let url = std::env::var(env).ok()?;
        assert!(url.ends_with(schema), "{env} must name {schema}");
        Some(Engine { url, schema })
    })
    .collect()
}

fn app(url: &str) -> (Router, MySqlManager) {
    let config = Config::from_values(&[]).unwrap();
    let manager = MySqlManager::new(url, &config.limits).unwrap();
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
    let (mut out, mut in_tag) = (String::new(), false);
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
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[tokio::test]
async fn each_engine_reports_authentication_and_network_failures_without_secrets() {
    for engine in engines() {
        let config = Config::from_values(&[]).unwrap();
        let parsed = url::Url::parse(&engine.url).unwrap();
        let scheme = parsed.scheme();
        let port = parsed.port().unwrap();
        let bad_password = format!(
            "{scheme}://cuadb_reader:not-the-password@127.0.0.1:{port}/{}",
            engine.schema
        );
        let bad_user = format!(
            "{scheme}://cuadb_no_such_user@127.0.0.1:{port}/{}",
            engine.schema
        );
        let unreachable = format!("{scheme}://cuadb_reader@127.0.0.1:1/{}", engine.schema);
        for (url, expected) in [
            (
                bad_password.as_str(),
                ConnectionStatus::AuthenticationFailed,
            ),
            (bad_user.as_str(), ConnectionStatus::AuthenticationFailed),
            (unreachable.as_str(), ConnectionStatus::Unavailable),
        ] {
            let manager = MySqlManager::new(url, &config.limits).unwrap();
            let status = manager.snapshot().await.status;
            assert_eq!(status, expected, "{} {url}", engine.schema);
            let snapshot = manager.snapshot().await;
            assert!(!snapshot.label().contains("not-the-password"));
            assert!(!snapshot.detail(None).contains("not-the-password"));
            manager.close().await;
        }
    }
}

#[tokio::test]
async fn each_engine_pages_and_sorts_with_nulls_last_and_a_stable_order() {
    for engine in engines() {
        let (router, manager) = app(&engine.url);
        let base = format!(
            "/catalog/data?schema={}&relation=Mixed_Items&size=1",
            engine.schema
        );
        let (status, first) = fetch(&router, &base).await;
        assert_eq!(status, StatusCode::OK, "{}", engine.schema);
        assert!(
            first.contains("1–1")
                && first.contains("<button type=\"button\" disabled>Previous</button>")
        );
        assert!(first.contains(">Next</a>"));
        let (_, second) = fetch(&router, &format!("{base}&page=1")).await;
        assert!(
            second.contains("2–2") && second.contains("disabled>Next<"),
            "{}",
            engine.schema
        );
        // `amount` is NULL on the second row: NULLs sort last in both directions.
        for dir in ["asc", "desc"] {
            let (_, one) = fetch(
                &router,
                &format!("{base}&page=1&sort=amount&direction={dir}"),
            )
            .await;
            assert!(one.contains("NULL"), "{} {dir}", engine.schema);
        }
        let (status, _) = fetch(&router, &format!("{base}&sort=payload&direction=asc")).await;
        assert!(status.is_client_error(), "binary columns are not sortable");
        manager.close().await;
    }
}

#[tokio::test]
async fn each_engine_renders_rich_values_exactly_escaped_and_bounded() {
    for engine in engines() {
        let (router, manager) = app(&engine.url);
        let (status, html) = fetch(
            &router,
            &format!(
                "/catalog/data?schema={}&relation=rich_values&size=25",
                engine.schema
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{}", engine.schema);
        assert!(!html.contains("<script>alert(1)"), "{}", engine.schema);
        assert!(
            !html.contains('\u{7}'),
            "control characters must be visible escapes"
        );
        assert!(html.contains("\\u{0007}"));
        assert!(
            html.contains("12345678901234567890.1234567890"),
            "exact decimal"
        );
        assert!(html.contains("2026-01-02 03:04:05.123456"), "datetime(6)");
        assert!(html.contains("12:34:56.789") && html.contains("2026-01-02"));
        assert!(html.contains("😀 日本 Ünï"), "utf8mb4 round trip");
        assert!(html.contains("00ff10"), "varbinary as hex");
        assert!(html.contains("a5"), "BIT(8) 10100101 renders as 0xa5");
        // 100 KB blob and 100 KB text rows must not inflate the page.
        assert!(
            html.len() < 120_000,
            "{} page was {} bytes",
            engine.schema,
            html.len()
        );
        assert!(html.contains("aria-label=\"truncated\""));
        manager.close().await;
    }
}

#[tokio::test]
async fn each_engine_filter_is_literal_not_a_like_pattern() {
    for engine in engines() {
        let (router, manager) = app(&engine.url);
        let path = |filter: &str| {
            format!(
                "/catalog/data?schema={}&relation=rich_values&size=25&filter={filter}",
                engine.schema
            )
        };
        let (_, exact) = fetch(&router, &path("row%2010")).await;
        assert!(
            exact.contains("1–1"),
            "{}: one literal match",
            engine.schema
        );
        for wildcard in ["row%201_", "%25", "_", "row%25"] {
            let (status, html) = fetch(&router, &path(wildcard)).await;
            assert_eq!(status, StatusCode::OK);
            assert!(
                html.contains("0–0"),
                "{} {wildcard} must not act as a wildcard",
                engine.schema
            );
        }
        manager.close().await;
    }
}

#[tokio::test]
async fn each_engine_handles_views_empty_tables_storage_engines_and_hidden_schemas() {
    for engine in engines() {
        let (router, manager) = app(&engine.url);
        let s = engine.schema;
        let (_, view) = fetch(
            &router,
            &format!("/catalog/structure?schema={s}&relation=items_view"),
        )
        .await;
        assert!(text(&view).contains("id") && text(&view).contains("label"));
        let (_, empty) = fetch(
            &router,
            &format!("/catalog/data?schema={s}&relation=empty_t"),
        )
        .await;
        assert!(empty.contains("0–0"));
        let (status, stats) = fetch(
            &router,
            &format!("/catalog/stats?schema={s}&relation=legacy_myisam"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{s}: MyISAM statistics");
        assert!(text(&stats).contains("Estimated rows"));
        let (_, indexes) = fetch(
            &router,
            &format!("/catalog/indexes?schema={s}&relation=Mixed_Items"),
        )
        .await;
        assert!(text(&indexes).contains("PRIMARY"));
        let (_, structure) = fetch(
            &router,
            &format!("/catalog/structure?schema={s}&relation=child"),
        )
        .await;
        assert!(text(&structure).contains("fk_item") && text(&structure).contains("Mixed_Items"));
        for hidden in ["mysql", "information_schema", "performance_schema", "sys"] {
            let (_, tree) = fetch(&router, &format!("/catalog/relations?schema={hidden}")).await;
            assert!(
                !text(&tree).contains("user"),
                "{s}: {hidden} relations leaked"
            );
        }
        let (_, schemas) = fetch(&router, "/catalog/schemas").await;
        assert!(schemas.contains(s) && !schemas.contains("performance_schema"));
        manager.close().await;
    }
}
