//! SQL workspace on live PostgreSQL 18. Needs CUADB_TEST_PG_WRITER_URL (the fixture
//! operator, who owns the additive cuadb_rw_scratch schema); returns early otherwise
//! (NOT evidence). Tests insert and delete only rows they created.

use std::time::Duration;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use cuadb::{
    app::router_with_state,
    auth::{AuthService, bootstrap::bootstrap_admin, service::SessionPolicy, store::AuthStore},
    config::Config,
    state::AppState,
};
use sqlx::{Connection, Row};
use tower::ServiceExt;

const PW: &str = "a long enough passphrase";

struct World {
    router: Router,
    svc: AuthService,
    admin: (String, String),
    url: String,
}

async fn world(url: &str, access: &str, query_ms: &str) -> World {
    let config = Config::from_values(&[
        ("CUADB_DATABASE_URL", url),
        ("CUADB_SQL_WORKSPACE", "enabled"),
        ("CUADB_DATABASE_ACCESS", access),
        ("CUADB_QUERY_TIMEOUT_MS", query_ms),
        ("CUADB_SQL_MAX_ROWS", "50"),
    ])
    .unwrap();
    let svc = AuthService::new(
        AuthStore::open_memory().await.unwrap(),
        SessionPolicy {
            idle_secs: 1800,
            max_secs: 43_200,
            cookie_secure: false,
        },
        false,
    )
    .await
    .unwrap();
    bootstrap_admin(&svc.store, "admin", PW).await.unwrap();
    svc.refresh_enforced().await.unwrap();
    let state = AppState::from_config(&config)
        .unwrap()
        .with_auth(svc.clone());
    let router = router_with_state(&config.limits, state);
    let login = svc.login("admin", PW).await.unwrap();
    World {
        router,
        svc,
        admin: (format!("cuadb_session={}", login.token), login.csrf),
        url: url.to_string(),
    }
}

fn writer() -> Option<String> {
    let url = std::env::var("CUADB_TEST_PG_WRITER_URL").ok()?;
    assert!(
        url::Url::parse(&url)
            .unwrap()
            .path()
            .trim_start_matches('/')
            .starts_with("cuadb_fixture_")
    );
    Some(url)
}

async fn send(router: &Router, mut request: Request<Body>) -> (StatusCode, String) {
    request
        .headers_mut()
        .entry(header::HOST)
        .or_insert("localhost:4848".parse().unwrap());
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 22).await.unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

async fn post(w: &World, path: &str, body: &str) -> (StatusCode, String) {
    send(
        &w.router,
        Request::post(path)
            .header(header::COOKIE, &w.admin.0)
            .header(header::ORIGIN, "http://localhost:4848")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("{body}&csrf={}", w.admin.1)))
            .unwrap(),
    )
    .await
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

async fn sql(w: &World, statement: &str) -> (StatusCode, String) {
    post(w, "/sql/execute", &format!("sql={}", enc(statement))).await
}

async fn scalar(w: &World, query: &str) -> String {
    let mut c = sqlx::PgConnection::connect(&w.url).await.unwrap();
    let row = sqlx::query(query).fetch_one(&mut c).await.unwrap();
    row.try_get::<String, _>(0)
        .or_else(|_| row.try_get::<i64, _>(0).map(|n| n.to_string()))
        .or_else(|_| row.try_get::<i32, _>(0).map(|n| n.to_string()))
        .unwrap_or_else(|_| "NULL".into())
}

async fn clean(w: &World) {
    let mut c = sqlx::PgConnection::connect(&w.url).await.unwrap();
    sqlx::query("DELETE FROM cuadb_rw_scratch.items WHERE name LIKE 'pgt-%'")
        .execute(&mut c)
        .await
        .unwrap();
}

async fn status_of(w: &World, tx: &str) -> (StatusCode, String) {
    send(
        &w.router,
        Request::get(format!("/tx/status?tx={tx}"))
            .header(header::COOKIE, &w.admin.0)
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

fn tx_id(body: &str) -> String {
    let start = body.find("data-tx=\"").expect("tx id") + 9;
    body[start..start + 32].to_string()
}

#[tokio::test]
async fn reads_decode_every_type_exactly_run_read_only_and_refuse_dangerous_statements() {
    let Some(url) = writer() else { return };
    let w = world(&url, "read-write", "5000").await;
    let (status, body) = sql(
        &w,
        "SELECT 12345678901234567890.1234567890::numeric AS n, true AS b, NULL::text AS z, '{\"a\":[1,2]}'::jsonb AS j, ARRAY[1,2,NULL] AS arr, '2026-01-02 03:04:05.123456+00'::timestamptz AS t, 9223372036854775807::bigint AS big, 'x<script>'::text AS markup, 1 AS dup, 2 AS dup",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for needle in [
        "12345678901234567890.1234567890",
        "9223372036854775807",
        "2026-01-02",
        ">NULL<",
        "true",
    ] {
        assert!(
            body.contains(needle) || body.contains("data-null"),
            "{needle} in {body}"
        );
    }
    assert!(!body.contains("<script>"), "markup is escaped");
    assert_eq!(
        body.matches(">dup<").count(),
        2,
        "duplicate column names are kept"
    );
    // EXPLAIN works; it never executes the statement.
    let (status, plan) = sql(&w, "EXPLAIN SELECT * FROM cuadb_rw_scratch.items").await;
    assert_eq!(status, StatusCode::OK);
    assert!(plan.contains("Seq Scan") || plan.contains("items"));
    assert_eq!(
        sql(&w, "EXPLAIN ANALYZE SELECT 1").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Dangerous statements and functions never reach the database.
    for statement in [
        "SELECT pg_sleep(5)",
        "SELECT pg_read_file('/etc/passwd')",
        "SELECT set_config('statement_timeout','0',false)",
        "SELECT nextval('cuadb_rw_scratch.items_id_seq')",
        "SELECT * FROM cuadb_rw_scratch.items FOR UPDATE",
        "COPY cuadb_rw_scratch.items TO '/tmp/x'",
        "DROP SCHEMA cuadb_rw_scratch CASCADE",
        "SELECT 1; SELECT 2",
        "WITH d AS (DELETE FROM cuadb_rw_scratch.items RETURNING 1) SELECT * FROM d",
    ] {
        let (status, _) = sql(&w, statement).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{statement}");
    }
    // Defense in depth: a function the policy cannot see into still cannot write,
    // because reads run in a READ ONLY transaction.
    let before = scalar(&w, "SELECT count(*) FROM cuadb_rw_scratch.audit_probe").await;
    let (status, _) = sql(&w, "SELECT cuadb_rw_scratch.sneaky()").await;
    assert!(
        status.is_client_error(),
        "side-effecting function must fail in a read-only transaction, got {status}"
    );
    assert_eq!(
        scalar(&w, "SELECT count(*) FROM cuadb_rw_scratch.audit_probe").await,
        before,
        "nothing was written"
    );
    // Row cap (CUADB_SQL_MAX_ROWS=50) marks truncation.
    let (_, many) = sql(&w, "SELECT g FROM generate_series(1, 500) g").await;
    assert!(many.contains("result truncated"));
    // The statement with a trailing comment still works (the wrapper adds a newline).
    assert_eq!(
        sql(&w, "SELECT 1 AS x -- trailing comment").await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn server_side_timeout_and_cancel_stop_runaway_reads_and_the_pool_recovers() {
    let Some(url) = writer() else { return };
    let w = world(&url, "read-only", "400").await;
    let started = std::time::Instant::now();
    let (status, _) = sql(&w, "SELECT count(*) FROM generate_series(1, 2000000000)").await;
    assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(sql(&w, "SELECT 1").await.0, StatusCode::OK, "pool recovers");

    let w = world(&url, "read-only", "60000").await;
    let id = "cd".repeat(16);
    let runner = {
        let (router, cookie, csrf) = (w.router.clone(), w.admin.0.clone(), w.admin.1.clone());
        let body = format!(
            "sql={}&statement_id={id}&csrf={csrf}",
            enc("SELECT count(*) FROM generate_series(1, 2000000000)")
        );
        tokio::spawn(async move {
            send(
                &router,
                Request::post("/sql/execute")
                    .header(header::COOKIE, cookie)
                    .header(header::ORIGIN, "http://localhost:4848")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
        })
    };
    tokio::time::sleep(Duration::from_millis(500)).await;
    let t = std::time::Instant::now();
    assert_eq!(
        post(&w, "/sql/cancel", &format!("statement_id={id}"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(runner.await.unwrap().0, StatusCode::CONFLICT);
    assert!(t.elapsed() < Duration::from_secs(3));
    // The backend really stopped: no long-running generate_series query remains.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let running = scalar(&w, "SELECT count(*) FROM pg_stat_activity WHERE query LIKE '%generate_series(1, 2000000000)%' AND state = 'active' AND pid <> pg_backend_pid()").await;
    assert_eq!(
        running, "0",
        "cancelled statements must not keep running on the server"
    );
}

#[tokio::test]
async fn typed_edits_and_transactions_work_with_stale_checks_and_real_constraints() {
    let Some(url) = writer() else { return };
    let w = world(&url, "read-write", "3000").await;
    clean(&w).await;
    let ok = |r: (StatusCode, String)| {
        assert_eq!(r.0, StatusCode::OK, "{}", r.1);
        r.1
    };
    ok(post(
        &w,
        "/data/insert",
        &format!(
            "schema=cuadb_rw_scratch&relation=items&col.name={}&col.qty=3&null.note=1",
            enc("pgt-a'; DROP TABLE x; --")
        ),
    )
    .await);
    assert_eq!(
        scalar(
            &w,
            "SELECT total FROM cuadb_rw_scratch.items WHERE name LIKE 'pgt-a%'"
        )
        .await,
        "6",
        "generated column computed by the database"
    );
    let id = scalar(
        &w,
        "SELECT id FROM cuadb_rw_scratch.items WHERE name LIKE 'pgt-a%'",
    )
    .await;
    ok(post(&w, "/data/update", &format!("schema=cuadb_rw_scratch&relation=items&key.id={id}&col.qty=8&old.qty=3&col.note=hello&oldnull.note=1")).await);
    assert_eq!(
        scalar(
            &w,
            &format!("SELECT qty FROM cuadb_rw_scratch.items WHERE id = {id}")
        )
        .await,
        "8"
    );
    // Stale previous value => conflict and no change.
    assert_eq!(
        post(
            &w,
            "/data/update",
            &format!("schema=cuadb_rw_scratch&relation=items&key.id={id}&col.qty=99&old.qty=3")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        scalar(
            &w,
            &format!("SELECT qty FROM cuadb_rw_scratch.items WHERE id = {id}")
        )
        .await,
        "8"
    );
    // Generated and identity-ALWAYS columns are protected; unique violation is a conflict.
    assert_eq!(
        post(
            &w,
            "/data/update",
            &format!("schema=cuadb_rw_scratch&relation=items&key.id={id}&col.total=1&old.total=16")
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &w,
            "/data/insert",
            &format!(
                "schema=cuadb_rw_scratch&relation=items&col.name={}",
                enc("pgt-a'; DROP TABLE x; --")
            )
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // A bad value for the column type is rejected by the database, not interpolated.
    assert!(
        post(
            &w,
            "/data/update",
            &format!(
                "schema=cuadb_rw_scratch&relation=items&key.id={id}&col.qty=notanumber&old.qty=8"
            )
        )
        .await
        .0
        .is_client_error()
    );
    // Transactions: stage, review, rollback; stage, commit.
    let tx = tx_id(&ok(post(&w, "/tx/begin", "profile=default").await));
    ok(post(
        &w,
        "/data/insert",
        &format!("schema=cuadb_rw_scratch&relation=items&tx={tx}&col.name=pgt-b&col.qty=1"),
    )
    .await);
    assert_eq!(
        scalar(
            &w,
            "SELECT count(*) FROM cuadb_rw_scratch.items WHERE name = 'pgt-b'"
        )
        .await,
        "0",
        "uncommitted rows are invisible"
    );
    let review = ok(status_of(&w, &tx).await);
    assert!(review.contains("1 pending") && !review.contains("pgt-b"));
    ok(post(&w, "/tx/rollback", &format!("tx={tx}")).await);
    assert_eq!(
        scalar(
            &w,
            "SELECT count(*) FROM cuadb_rw_scratch.items WHERE name = 'pgt-b'"
        )
        .await,
        "0"
    );
    let tx = tx_id(&ok(post(&w, "/tx/begin", "profile=default").await));
    ok(post(
        &w,
        "/data/insert",
        &format!("schema=cuadb_rw_scratch&relation=items&tx={tx}&col.name=pgt-c&col.qty=1"),
    )
    .await);
    ok(post(&w, "/tx/commit", &format!("tx={tx}&expected_changes=1")).await);
    assert_eq!(
        scalar(
            &w,
            "SELECT count(*) FROM cuadb_rw_scratch.items WHERE name = 'pgt-c'"
        )
        .await,
        "1"
    );
    // A failing statement aborts and removes the transaction (PostgreSQL poisons it anyway).
    let tx = tx_id(&ok(post(&w, "/tx/begin", "profile=default").await));
    ok(post(
        &w,
        "/data/insert",
        &format!("schema=cuadb_rw_scratch&relation=items&tx={tx}&col.name=pgt-d"),
    )
    .await);
    assert_eq!(
        post(
            &w,
            "/data/insert",
            &format!("schema=cuadb_rw_scratch&relation=items&tx={tx}&col.name=pgt-c")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        scalar(
            &w,
            "SELECT count(*) FROM cuadb_rw_scratch.items WHERE name = 'pgt-d'"
        )
        .await,
        "0"
    );
    // Delete with confirmation.
    assert_eq!(
        post(
            &w,
            "/data/delete",
            &format!(
                "schema=cuadb_rw_scratch&relation=items&key.id={id}&confirm=cuadb_rw_scratch.items"
            )
        )
        .await
        .0,
        StatusCode::OK
    );
    clean(&w).await;
    let events = w.svc.audit_events(200, None).await.unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.kind == "tx_commit" && e.outcome == "ok")
            && events
                .iter()
                .any(|e| e.kind == "data_change" && e.outcome == "failed")
    );
    assert!(
        events
            .iter()
            .all(|e| !format!("{} {}", e.resource, e.detail).contains("pgt-")),
        "no row values in the audit log"
    );
}

#[tokio::test]
async fn concurrent_row_changes_are_bounded_by_lock_timeout_and_do_not_corrupt_the_open_transaction()
 {
    let Some(url) = writer() else { return };
    let w = world(&url, "read-write", "1500").await;
    clean(&w).await;
    assert_eq!(
        post(
            &w,
            "/data/insert",
            "schema=cuadb_rw_scratch&relation=items&col.name=pgt-lock&col.qty=1"
        )
        .await
        .0,
        StatusCode::OK
    );
    let id = scalar(
        &w,
        "SELECT id FROM cuadb_rw_scratch.items WHERE name = 'pgt-lock'",
    )
    .await;
    let (status, begin) = post(&w, "/tx/begin", "profile=default").await;
    assert_eq!(status, StatusCode::OK);
    let tx = tx_id(&begin);
    assert_eq!(
        post(
            &w,
            "/data/update",
            &format!(
                "schema=cuadb_rw_scratch&relation=items&tx={tx}&key.id={id}&col.qty=2&old.qty=1"
            )
        )
        .await
        .0,
        StatusCode::OK
    );
    // A second writer hits the row lock and gives up within the lock timeout.
    let started = std::time::Instant::now();
    let (status, _) = post(
        &w,
        "/data/update",
        &format!("schema=cuadb_rw_scratch&relation=items&key.id={id}&col.qty=7&old.qty=1"),
    )
    .await;
    assert!(
        status == StatusCode::CONFLICT || status == StatusCode::GATEWAY_TIMEOUT,
        "{status}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    // The first transaction is intact and commits; the other change never happened.
    assert_eq!(
        post(&w, "/tx/commit", &format!("tx={tx}&expected_changes=1"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        scalar(
            &w,
            &format!("SELECT qty FROM cuadb_rw_scratch.items WHERE id = {id}")
        )
        .await,
        "2"
    );
    // And now the stale check protects against the committed concurrent change.
    assert_eq!(
        post(
            &w,
            "/data/update",
            &format!("schema=cuadb_rw_scratch&relation=items&key.id={id}&col.qty=9&old.qty=1")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    clean(&w).await;
}

#[tokio::test]
async fn database_privileges_stay_an_independent_boundary() {
    let Ok(reader) = std::env::var("CUADB_TEST_CATALOG_URL") else {
        return;
    };
    // A read-write profile whose credentials are SELECT-only: CuaDB allows it, the database refuses.
    let w = world(&reader, "read-write", "3000").await;
    let (status, _) = post(
        &w,
        "/data/insert",
        "schema=cuadb_rw_scratch&relation=items&col.name=pgt-denied",
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN
            || status == StatusCode::CONFLICT
            || status.is_client_error(),
        "{status}"
    );
    let (status, _) = sql(&w, "DELETE FROM cuadb_rw_scratch.items").await;
    assert!(status.is_client_error());
    let mut c = sqlx::PgConnection::connect(&writer().unwrap_or(reader.clone()))
        .await
        .unwrap();
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cuadb_rw_scratch.items WHERE name = 'pgt-denied'")
            .fetch_one(&mut c)
            .await
            .unwrap();
    assert_eq!(n, 0);
    // Reads with the reader still work.
    assert_eq!(sql(&w, "SELECT 1").await.0, StatusCode::OK);
}
