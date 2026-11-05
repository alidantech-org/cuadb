//! PostgreSQL adapter acceptance tests.
//!
//! Ordinary tests never mutate data or require a database. Live tests run only
//! with explicit CUADB_TEST_* URLs pointing at disposable fixtures.

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{
    app,
    config::Config,
    db::{postgres::PgManager, types::ConnectionStatus},
    state::AppState,
};
use tower::ServiceExt;

#[tokio::test]
async fn disconnected_server_responds_without_a_database() {
    let sqlite_root = std::env::temp_dir().join(format!("cuadb-no-config-{}", std::process::id()));
    let config =
        Config::from_values(&[("CUADB_SQLITE_ROOT", sqlite_root.to_str().unwrap())]).unwrap();
    let state = AppState::from_config(&config).await.unwrap();
    let response = app::router_with_state(&config.limits, state.clone())
        .oneshot(
            Request::builder()
                .uri("/connection/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Connected"));
    assert!(!html.contains("postgresql://"));
    state.close().await;
    let _ = std::fs::remove_dir_all(sqlite_root);
}

#[tokio::test]
async fn configured_but_unreachable_database_does_not_break_health() {
    let config = Config::from_values(&[
        (
            "CUADB_DATABASE_URL",
            "postgresql://reader:secret@127.0.0.1:1/unreachable?sslmode=disable",
        ),
        ("CUADB_CONNECTION_TIMEOUT_MS", "100"),
        ("CUADB_QUERY_TIMEOUT_MS", "100"),
    ])
    .unwrap();

    let state = AppState::from_config(&config).await.unwrap();
    let health = app::router_with_state(&config.limits, state.clone())
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let snapshot = state.connection_snapshot().await;
    assert_eq!(snapshot.status, ConnectionStatus::Unavailable);
    assert!(snapshot.error_code.is_some());
    assert!(!format!("{snapshot:?}").contains("secret"));

    let status = app::router_with_state(&config.limits, state.clone())
        .oneshot(
            Request::builder()
                .uri("/connection/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let body = to_bytes(status.into_body(), 4096).await.unwrap();
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("Unavailable")
    );
    state.close().await;
}

/// Opt-in read-only verification for a localhost PostgreSQL installation
/// using trust authentication. This executes only connection bootstrap and
/// SELECT checks; it does not create or modify database objects.
#[tokio::test]
async fn local_passwordless_postgres_can_pass_the_read_only_probe() {
    let Ok(url) = std::env::var("CUADB_TEST_LOCAL_TRUST_URL") else {
        return;
    };
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(&url, &config.limits).unwrap();
    let snapshot = manager.snapshot().await;
    assert_eq!(snapshot.status, ConnectionStatus::Ready);
    assert!(manager.read_only_session_verified().await.unwrap());
    manager.close().await;
}

/// Only runs against a disposable PostgreSQL database with a SELECT-only role.
/// The test itself never creates, modifies, truncates or deletes any data.
#[tokio::test]
async fn live_postgres_fixture_read_only_probe() {
    let Ok(url) = std::env::var("CUADB_TEST_POSTGRES_URL") else {
        return;
    };
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(&url, &config.limits).unwrap();
    let snapshot = manager.snapshot().await;
    assert_eq!(
        snapshot.status,
        ConnectionStatus::Ready,
        "Use a reachable disposable fixture and a role with schema USAGE"
    );
    assert!(manager.read_only_session_verified().await.unwrap());
    manager.close().await;
}

/// Optional fixture must use password authentication; peer/trust won't reject
/// deliberately wrong credentials. Nothing is written.
#[tokio::test]
async fn live_postgres_fixture_rejects_wrong_password() {
    let Ok(url) = std::env::var("CUADB_TEST_POSTGRES_URL") else {
        return;
    };
    let mut parsed = url::Url::parse(&url).unwrap();
    parsed
        .set_password(Some("cuadb-incorrect-fixture-password"))
        .unwrap();
    let config = Config::from_values(&[
        ("CUADB_CONNECTION_TIMEOUT_MS", "1000"),
        ("CUADB_QUERY_TIMEOUT_MS", "1000"),
    ])
    .unwrap();
    let manager = PgManager::new(parsed.as_str(), &config.limits).unwrap();
    let snapshot = manager.snapshot().await;
    assert_eq!(snapshot.status, ConnectionStatus::AuthenticationFailed);
    manager.close().await;
}

/// Optional restricted fixture with a role lacking USAGE on every non-system
/// schema. Permissions are prepared out-of-band by a fixture admin.
#[tokio::test]
async fn live_postgres_fixture_detects_missing_schema_privileges() {
    let Ok(url) = std::env::var("CUADB_TEST_DENIED_POSTGRES_URL") else {
        return;
    };
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(&url, &config.limits).unwrap();
    assert_eq!(
        manager.snapshot().await.status,
        ConnectionStatus::PermissionDenied
    );
    manager.close().await;
}
