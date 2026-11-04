//! Router contracts exercised against an in-memory adapter: no database, no sockets.

use std::{sync::Arc, time::Duration};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{
    app::router_with_state,
    config::Config,
    db::{
        adapter::DatabaseAdapter,
        types::{
            Capability, Column, DbError, DbErrorKind, Engine, Namespace, PageRequest, Relation,
            RelationId, RelationKind, RelationStats, RowPage,
        },
    },
    state::AppState,
};
use tower::ServiceExt;

#[derive(Clone, Copy)]
enum Mode {
    Healthy,
    Denied,
    Slow,
}

struct Fake(Mode);

#[async_trait::async_trait]
impl DatabaseAdapter for Fake {
    fn engine(&self) -> Engine {
        Engine::Sqlite
    }
    fn supports(&self, _: Capability) -> bool {
        false
    }
    async fn namespaces(&self) -> Result<Vec<Namespace>, DbError> {
        match self.0 {
            Mode::Healthy => Ok(vec![Namespace {
                name: "<script>alert(1)</script>".into(),
            }]),
            Mode::Denied => Err(DbError::new(DbErrorKind::PermissionDenied)),
            Mode::Slow => {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(vec![])
            }
        }
    }
    async fn relations(&self, _: &str) -> Result<Vec<Relation>, DbError> {
        match self.0 {
            Mode::Healthy => Ok(vec![Relation {
                id: RelationId {
                    namespace: "main".into(),
                    name: "users".into(),
                },
                kind: RelationKind::Table,
            }]),
            _ => Err(DbError::new(DbErrorKind::PermissionDenied)),
        }
    }
    async fn columns(&self, _: &RelationId) -> Result<Vec<Column>, DbError> {
        Ok(vec![])
    }
    async fn statistics(&self, _: &RelationId) -> Result<RelationStats, DbError> {
        Err(DbError::new(DbErrorKind::Unsupported))
    }
    async fn rows(&self, _: &RelationId, _: PageRequest) -> Result<RowPage, DbError> {
        Err(DbError::new(DbErrorKind::Unsupported))
    }
}

async fn get(mode: Mode, path: &str, timeout_ms: u64) -> (StatusCode, String) {
    let mut limits = Config::from_values(&[]).expect("config").limits;
    limits.request_timeout = Duration::from_millis(timeout_ms);
    let app = router_with_state(&limits, AppState::with_adapter(Arc::new(Fake(mode))));
    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn schema_tree_escapes_names_from_the_adapter() {
    let (status, body) = get(Mode::Healthy, "/catalog/schemas", 2000).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("<script>alert(1)"));
    assert!(body.contains("&lt;script&gt;") || body.contains("&#60;script&#62;"));
}

#[tokio::test]
async fn relations_route_lists_adapter_relations() {
    let (status, body) = get(Mode::Healthy, "/catalog/relations?schema=main", 2000).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("users"));
}

#[tokio::test]
async fn permission_denied_is_a_safe_in_region_fragment() {
    let (status, body) = get(Mode::Denied, "/catalog/schemas", 2000).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Insufficient permissions"));
}

#[tokio::test]
async fn permission_failure_on_relations_does_not_leak_driver_text() {
    let (status, body) = get(Mode::Denied, "/catalog/relations?schema=main", 2000).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!body.to_lowercase().contains("postgres"));
}

#[tokio::test]
async fn slow_adapter_is_cut_off_by_the_request_timeout() {
    let started = std::time::Instant::now();
    let (status, _) = get(Mode::Slow, "/catalog/schemas", 100).await;
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        status == StatusCode::REQUEST_TIMEOUT
            || status == StatusCode::GATEWAY_TIMEOUT
            || status == StatusCode::SERVICE_UNAVAILABLE,
        "{status}"
    );
}
