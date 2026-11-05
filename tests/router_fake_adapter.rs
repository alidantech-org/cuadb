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
            Availability, Capability, CatalogList, Cell, Column, Constraint, ConstraintKind,
            DbError, DbErrorKind, Engine, Namespace, PageRequest, Relation, RelationId,
            RelationKind, RelationStats, RowOptions, RowPage,
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
            Mode::Healthy => Ok([
                "users",
                "Users_Archive",
                "50%_off",
                "<b>x</b>",
                "orders",
                "units",
            ]
            .into_iter()
            .map(|name| Relation {
                id: RelationId {
                    namespace: "main".into(),
                    name: name.into(),
                },
                kind: RelationKind::Table,
            })
            .collect()),
            _ => Err(DbError::new(DbErrorKind::PermissionDenied)),
        }
    }
    async fn columns(&self, _: &RelationId) -> Result<Vec<Column>, DbError> {
        Ok(vec![])
    }
    async fn statistics(&self, _: &RelationId) -> Result<RelationStats, DbError> {
        Err(DbError::new(DbErrorKind::Unsupported))
    }
    async fn page(&self, _: &RelationId, options: RowOptions) -> Result<RowPage, DbError> {
        if matches!(self.0, Mode::Denied) {
            return Err(DbError::new(DbErrorKind::PermissionDenied));
        }
        if matches!(self.0, Mode::Slow) {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        // 250 rows in pages of options.limit, addressed by page number.
        let start = (options.page as usize) * options.limit as usize;
        let end = (start + options.limit as usize).min(250);
        let col = |name: &str| Column {
            name: name.into(),
            native_type: "text".into(),
            nullable: true,
            primary_key: false,
            default_expression: None,
        };
        Ok(RowPage {
            available_columns: vec![col("id"), col("note")],
            columns: vec![col("id"), col("note")],
            rows: (start..end)
                .map(|i| vec![Cell::Integer(i as i64), Cell::Text(format!("=cmd{i}"))])
                .collect(),
            has_more: end < 250,
            approximate_total: Availability::Unavailable,
            previews: vec![],
            has_previous: start > 0,
            next_cursor: None,
            previous_cursor: None,
            stable_order: true,
            keyset_enabled: false,
            page: options.page,
            offset: start as u64,
            truncated_columns: false,
        })
    }
    async fn list_relations(&self, namespace: &str) -> Result<CatalogList<Relation>, DbError> {
        if namespace != "big" {
            return Ok(CatalogList {
                items: self.relations(namespace).await?,
                truncated: false,
            });
        }
        Ok(CatalogList {
            items: (0..500)
                .map(|i| Relation {
                    id: RelationId {
                        namespace: "big".into(),
                        name: format!("table_{i:03}"),
                    },
                    kind: RelationKind::Table,
                })
                .collect(),
            truncated: true,
        })
    }
    async fn constraints(&self, id: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        let items = if id.name == "t" {
            vec![Constraint {
                name: "t_u_fk".into(),
                kind: ConstraintKind::ForeignKey,
                columns: vec!["id".into()],
                references: Some(RelationId {
                    namespace: "main".into(),
                    name: "u".into(),
                }),
                referenced_columns: vec!["id".into()],
                definition: String::new(),
            }]
        } else {
            vec![]
        };
        Ok(CatalogList {
            items,
            truncated: false,
        })
    }
    async fn rows(&self, _: &RelationId, _: PageRequest) -> Result<RowPage, DbError> {
        Err(DbError::new(DbErrorKind::Unsupported))
    }
}

async fn get_bytes(mode: Mode, path: &str) -> (StatusCode, Vec<u8>) {
    let limits = Config::from_values(&[]).expect("config").limits;
    let app = router_with_state(&limits, AppState::with_adapter(Arc::new(Fake(mode))));
    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    (
        status,
        to_bytes(response.into_body(), 1 << 22)
            .await
            .unwrap()
            .to_vec(),
    )
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
    assert!(body.contains("data-details=\"false\""));
    assert!(!body.contains("Loading relations"));
}

#[tokio::test]
async fn relations_route_lists_adapter_relations() {
    let (status, body) = get(Mode::Healthy, "/catalog/relations?schema=main", 2000).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("users"));
    assert!(body.contains("data-details=\"false\""));
    assert!(!body.contains("data-count"));
    let (_, detailed) = get(
        Mode::Healthy,
        "/catalog/relations?schema=main&details=true",
        2000,
    )
    .await;
    assert!(detailed.contains("data-details=\"true\""));
}

#[tokio::test]
async fn relation_search_returns_exact_names_without_waiting_for_statistics() {
    let (status, body) = get(Mode::Healthy, "/catalog/relations?schema=main&q=unit", 2000).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("units"));
    assert!(!body.contains("users"));
    assert!(body.contains("data-details=\"false\""));
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

#[tokio::test]
async fn csv_export_of_one_page_is_bounded_and_formula_safe() {
    let (status, body) = get(
        Mode::Healthy,
        "/catalog/export?schema=main&relation=t&format=csv&size=3",
        2000,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let lines: Vec<&str> = body.trim_end().split("\r\n").collect();
    assert_eq!(lines[0], "id,note");
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[1], "0,\"'=cmd0\"");
}

#[tokio::test]
async fn relation_scope_walks_pages_and_json_reports_completeness() {
    let (status, body) = get(
        Mode::Healthy,
        "/catalog/export?schema=main&relation=t&format=json&scope=relation",
        5000,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let doc: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(doc["rows"].as_array().unwrap().len(), 250);
    assert_eq!(doc["truncated"], false);
}

#[tokio::test]
async fn export_rejects_unknown_formats_scopes_and_browser_supplied_targets() {
    for path in [
        "/catalog/export?schema=main&relation=t&format=xml",
        "/catalog/export?schema=main&relation=t&format=csv&scope=all",
        "/catalog/export?schema=main&relation=t&format=csv&scope=relation&page=1",
        "/catalog/export?schema=main&relation=t&format=csv&size=1001",
        "/catalog/export?schema=main&relation=t&format=csv&sql=select+1",
        "/catalog/export?schema=main&relation=t&format=csv&url=postgres://x",
    ] {
        let (status, _) = get(Mode::Healthy, path, 2000).await;
        assert!(status.is_client_error(), "{path} -> {status}");
    }
}

#[tokio::test]
async fn export_headers_are_attachment_no_store_and_permission_errors_are_403() {
    let config = Config::from_values(&[]).unwrap();
    let app = router_with_state(
        &config.limits,
        AppState::with_adapter(Arc::new(Fake(Mode::Healthy))),
    );
    let r = app
        .oneshot(
            Request::get("/catalog/export?schema=a%22b&relation=t&format=csv")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.headers()["cache-control"], "no-store");
    assert_eq!(r.headers()["content-type"], "text/csv; charset=utf-8");
    assert_eq!(
        r.headers()["content-disposition"],
        "attachment; filename=\"a_b_t.csv\""
    );
    let (status, _) = get(
        Mode::Denied,
        "/catalog/export?schema=main&relation=t&format=csv",
        2000,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn data_fragment_synchronizes_its_own_requests_so_stale_replies_are_dropped() {
    let (status, body) = get(
        Mode::Healthy,
        "/catalog/data?schema=main&relation=t&size=25",
        2000,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // The panel root cancels an in-flight request when a newer sort/page/filter starts.
    assert!(body.contains(r#"id="catalog-panel""#));
    assert!(body.contains(r#"hx-sync="this:replace""#));
    assert!(body.contains(r#"hx-history="false""#));
}

#[tokio::test]
async fn failures_have_the_same_in_region_fragment_for_full_page_and_htmx_requests() {
    let config = Config::from_values(&[]).unwrap();
    let mut bodies = vec![];
    for htmx in [false, true] {
        let app = router_with_state(
            &config.limits,
            AppState::with_adapter(Arc::new(Fake(Mode::Denied))),
        );
        let mut request = Request::get("/catalog/data?schema=main&relation=t");
        if htmx {
            request = request.header("hx-request", "true");
        }
        let response = app
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()["x-cuadb-error"], "access_denied");
        assert_eq!(response.headers()["cache-control"], "no-store");
        bodies.push(to_bytes(response.into_body(), 1 << 20).await.unwrap());
    }
    assert_eq!(bodies[0], bodies[1]);
    let text = String::from_utf8_lossy(&bodies[0]);
    assert!(text.contains(r#"class="cuadb-error""#) && text.contains("role=\"alert\""));
}

#[tokio::test]
async fn slow_data_requests_time_out_with_a_gateway_status_and_error_fragment() {
    let config = Config::from_values(&[]).unwrap();
    let mut limits = config.limits;
    limits.request_timeout = Duration::from_millis(50);
    let app = router_with_state(&limits, AppState::with_adapter(Arc::new(Fake(Mode::Slow))));
    let response = app
        .oneshot(
            Request::get("/catalog/data?schema=main&relation=t")
                .header("hx-request", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_server_error());
    assert_eq!(response.headers()["cache-control"], "no-store");
}

fn zip_names(bytes: Vec<u8>) -> Vec<String> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut names: Vec<String> = archive.file_names().map(String::from).collect();
    names.sort();
    names
}

#[tokio::test]
async fn related_zip_follows_foreign_keys_within_the_requested_depth() {
    let base = "/catalog/export?schema=main&relation=t&format=zip&scope=related";
    let (status, bytes) = get_bytes(Mode::Healthy, base).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        zip_names(bytes),
        ["main_t.csv", "main_u.csv", "manifest.json"]
    );
    let (_, bytes) = get_bytes(Mode::Healthy, &format!("{base}&depth=0")).await;
    assert_eq!(zip_names(bytes), ["main_t.csv", "manifest.json"]);
}

#[tokio::test]
async fn related_zip_manifest_lists_relationships_and_enforces_bounds() {
    use std::io::Read;
    let (_, bytes) = get_bytes(
        Mode::Healthy,
        "/catalog/export?schema=main&relation=t&format=zip&scope=related",
    )
    .await;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["relationships"][0]["to"], "main.u");
    assert_eq!(doc["relations"][0]["rows"], 250);
    for path in [
        "/catalog/export?schema=main&relation=t&format=zip",
        "/catalog/export?schema=main&relation=t&format=zip&scope=related&depth=3",
        "/catalog/export?schema=main&relation=t&format=zip&scope=related&sort=id&direction=asc",
        "/catalog/export?schema=main&relation=t&format=csv&depth=1",
    ] {
        let (status, _) = get_bytes(Mode::Healthy, path).await;
        assert!(status.is_client_error(), "{path} -> {status}");
    }
    let (status, _) = get_bytes(
        Mode::Denied,
        "/catalog/export?schema=main&relation=t&format=zip&scope=related",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn relation_search_is_literal_case_insensitive_bounded_and_escaped() {
    let base = "/catalog/relations?schema=main";
    let (_, body) = get(Mode::Healthy, &format!("{base}&q=USERS"), 2000).await;
    assert!(body.contains("users") && body.contains("Users_Archive") && !body.contains("orders"));
    // % and _ are literal characters, never SQL wildcards.
    let (_, body) = get(Mode::Healthy, &format!("{base}&q=%25_"), 2000).await;
    assert!(body.contains("50%_off") && !body.contains("orders") && !body.contains("users"));
    let (_, body) = get(Mode::Healthy, &format!("{base}&q=%3Cb%3E"), 2000).await;
    assert!(
        !body.contains("<b>x</b>") && (body.contains("&lt;b&gt;") || body.contains("&#60;b&#62;"))
    );
    let (_, body) = get(Mode::Healthy, &format!("{base}&q=zzz"), 2000).await;
    assert!(!body.contains("users"));
    let (_, body) = get(Mode::Healthy, &format!("{base}&q="), 2000).await;
    assert!(body.contains("orders") && body.contains("users"));
    for bad in [
        format!("{base}&q={}", "a".repeat(65)),
        format!("{base}&q=a%0Ab"),
    ] {
        let (status, _) = get(Mode::Healthy, &bad, 2000).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
}

#[tokio::test]
async fn many_relations_load_in_pages_and_search_covers_the_full_catalog() {
    let (status, body) = get(Mode::Healthy, "/catalog/relations?schema=big", 5000).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("table_199"));
    assert!(!body.contains("table_200"));
    assert!(body.contains("Load more tables"));
    let (_, body) = get(
        Mode::Healthy,
        "/catalog/relations?schema=big&offset=200&limit=200",
        5000,
    )
    .await;
    assert!(body.contains("table_200") && body.contains("table_399"));
    let (_, body) = get(
        Mode::Healthy,
        "/catalog/relations?schema=big&q=TABLE_499",
        5000,
    )
    .await;
    assert!(body.contains("table_499") && !body.contains("table_300"));
    let (_, body) = get(
        Mode::Healthy,
        "/catalog/relations?schema=big&q=table_49x",
        5000,
    )
    .await;
    assert!(
        body.contains("table_499"),
        "typo tolerant search should find nearby names"
    );
}

#[tokio::test]
async fn repeated_column_parameters_are_accepted_but_other_duplicates_are_not() {
    for path in [
        "/catalog/data?schema=main&relation=t&column=id&column=note&pick=1",
        "/catalog/export?schema=main&relation=t&format=csv&column=id&column=note",
    ] {
        let (status, _) = get(Mode::Healthy, path, 2000).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
    for path in [
        "/catalog/data?schema=main&schema=other&relation=t",
        "/catalog/data?schema=main&relation=t&size=25&size=50",
        "/catalog/export?schema=main&relation=t&format=csv&format=json",
        "/catalog/data?schema=main&relation=t&unknown=1",
    ] {
        let (status, _) = get(Mode::Healthy, path, 2000).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
}

/// Regression: the status fragment replaces itself (`outerHTML`). If it also
/// triggered on `load`, every swap would request /connection/status again, forever.
#[tokio::test]
async fn status_fragment_never_retriggers_itself_on_load() {
    let (status, body) = get(Mode::Healthy, "/connection/status", 2000).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"hx-trigger="click""#));
    assert!(
        !body.contains("load"),
        "a self-replacing fragment must not trigger on load"
    );
    let (_, page) = get(Mode::Healthy, "/", 2000).await;
    assert!(page.matches(r#"hx-trigger="load""#).count() >= 1);
}
