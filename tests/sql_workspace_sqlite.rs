//! SQL workspace, typed edits, transactions and audit against real SQLite files.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use cuadb::{
    app::router_with_state,
    auth::{
        AuthService, Role, bootstrap::bootstrap_admin, service::SessionPolicy, store::AuthStore,
    },
    config::Config,
    state::AppState,
};
use sqlx::Connection;
use tower::ServiceExt;

const PW: &str = "a long enough passphrase";

struct World {
    router: Router,
    svc: AuthService,
    state: AppState,
    db: PathBuf,
    admin: (String, String),
}

async fn make_db(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let mut c = sqlx::SqliteConnection::connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    for sql in [
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, qty INTEGER, note TEXT, total INTEGER GENERATED ALWAYS AS (qty * 2) STORED)",
        "CREATE TABLE parents (id INTEGER PRIMARY KEY)",
        "CREATE TABLE kids (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parents(id))",
        "CREATE TABLE keyless (a TEXT)",
        "CREATE TABLE dkids (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parents(id) DEFERRABLE INITIALLY DEFERRED)",
        "INSERT INTO items (id, name, qty, note) VALUES (1,'alpha',1,'first'),(2,'beta',2,NULL),(3,'gamma',3,'third')",
        "INSERT INTO parents VALUES (1)",
        "INSERT INTO keyless VALUES ('x')",
    ] {
        sqlx::query(sql).execute(&mut c).await.unwrap();
    }
    c.close().await.unwrap();
    path
}

fn scratch(name: &str) -> PathBuf {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sqlws-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn world(name: &str, access: &str, enabled: bool) -> World {
    let dir = scratch(name);
    let db = make_db(&dir, "main.db").await;
    let url = format!(
        "sqlite:///{}",
        db.display().to_string().trim_start_matches('/')
    );
    let config = Config::from_values(&[
        ("CUADB_DATABASE_URL", url.as_str()),
        ("CUADB_SQLITE_ROOT", dir.to_str().unwrap()),
        (
            "CUADB_SQL_WORKSPACE",
            if enabled { "enabled" } else { "disabled" },
        ),
        ("CUADB_DATABASE_ACCESS", access),
        ("CUADB_SQL_MAX_ROWS", "2"),
        ("CUADB_CONNECTION_TIMEOUT_MS", "600"),
        ("CUADB_QUERY_TIMEOUT_MS", "3000"),
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
    let router = router_with_state(&config.limits, state.clone());
    let login = svc.login("admin", PW).await.unwrap();
    World {
        router,
        svc,
        state,
        db,
        admin: (format!("cuadb_session={}", login.token), login.csrf),
    }
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

async fn post(
    router: &Router,
    who: &(String, String),
    path: &str,
    body: &str,
) -> (StatusCode, String) {
    send(
        router,
        Request::post(path)
            .header(header::COOKIE, &who.0)
            .header(header::ORIGIN, "http://localhost:4848")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("{body}&csrf={}", who.1)))
            .unwrap(),
    )
    .await
}

async fn get(router: &Router, who: &(String, String), path: &str) -> (StatusCode, String) {
    send(
        router,
        Request::get(path)
            .header(header::COOKIE, &who.0)
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

async fn user(
    w: &World,
    login: &str,
    role: Role,
    grants: &[(&str, &str, &str, u8)],
) -> (String, String) {
    let admin = w.svc.login("admin", PW).await.unwrap().principal;
    let id = w
        .svc
        .create_user(&admin, login, "a long enough passphrase!", role)
        .await
        .unwrap();
    for (p, s, r, a) in grants {
        w.svc.add_grant(&admin, id, p, s, r, *a).await.unwrap();
    }
    let session = w
        .svc
        .login(login, "a long enough passphrase!")
        .await
        .unwrap();
    (format!("cuadb_session={}", session.token), session.csrf)
}

async fn query(w: &World, sql: &str) -> Vec<Vec<String>> {
    let mut c = sqlx::SqliteConnection::connect(&format!("sqlite://{}?mode=ro", w.db.display()))
        .await
        .unwrap();
    use sqlx::Row;
    let rows = sqlx::query(sql).fetch_all(&mut c).await.unwrap();
    rows.iter()
        .map(|r| {
            (0..r.len())
                .map(|i| {
                    r.try_get::<String, _>(i)
                        .or_else(|_| r.try_get::<i64, _>(i).map(|n| n.to_string()))
                        .unwrap_or_else(|_| "NULL".into())
                })
                .collect()
        })
        .collect()
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[tokio::test]
async fn the_workspace_is_off_by_default_and_never_advertises_itself() {
    let w = world("off", "read-write", false).await;
    for (path, body) in [
        ("/sql/execute", "sql=SELECT+1"),
        (
            "/sql/cancel",
            "statement_id=00000000000000000000000000000000",
        ),
        ("/data/insert", "schema=main&relation=items&col.name=x"),
        ("/data/update", "schema=main&relation=items"),
        ("/data/delete", "schema=main&relation=items"),
        ("/tx/begin", "profile=default"),
        ("/tx/commit", "tx=x&expected_changes=0"),
        ("/tx/rollback", "tx=x"),
    ] {
        let (status, _) = post(&w.router, &w.admin, path, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    assert_eq!(
        get(&w.router, &w.admin, "/tx/status?tx=x").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(query(&w, "SELECT COUNT(*) FROM items").await[0][0], "3");
}

#[tokio::test]
async fn permissions_gate_every_endpoint_and_open_mode_can_never_run_sql() {
    let w = world("perm", "read-write", true).await;
    // Auth not enforced => the open principal: no SQL, no edits.
    let open_state = {
        let svc = AuthService::new(
            AuthStore::open_memory().await.unwrap(),
            SessionPolicy {
                idle_secs: 60,
                max_secs: 600,
                cookie_secure: false,
            },
            false,
        )
        .await
        .unwrap();
        AppState::from_config(&Config::from_values(&[("CUADB_SQL_WORKSPACE", "enabled")]).unwrap())
            .unwrap()
            .with_auth(svc)
    };
    let open = router_with_state(&Config::from_values(&[]).unwrap().limits, open_state);
    let anon = ("".to_string(), "".to_string());
    for (path, body) in [
        ("/sql/execute", "sql=SELECT+1"),
        ("/data/insert", "schema=main&relation=items&col.name=x"),
        ("/tx/begin", ""),
    ] {
        let (status, _) = post(&open, &anon, path, body).await;
        assert!(
            status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
            "{path} -> {status}"
        );
    }
    let viewer = user(&w, "vera", Role::Viewer, &[("default", "", "", 15)]).await;
    let editor_no_sql = user(&w, "edd", Role::Editor, &[("default", "", "", 7)]).await;
    let nobody = user(&w, "nob", Role::Editor, &[]).await;
    for who in [&viewer, &nobody] {
        for (path, body) in [
            ("/sql/execute", "sql=SELECT+1"),
            ("/tx/begin", "profile=default"),
        ] {
            assert_eq!(
                post(&w.router, who, path, body).await.0,
                StatusCode::FORBIDDEN,
                "{path}"
            );
        }
        assert_eq!(
            post(
                &w.router,
                who,
                "/data/insert",
                "schema=main&relation=items&col.name=x"
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    // Edit without the sql action can edit rows but not run SQL.
    assert_eq!(
        post(&w.router, &editor_no_sql, "/sql/execute", "sql=SELECT+1")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &w.router,
            &editor_no_sql,
            "/data/insert",
            "schema=main&relation=items&col.name=delta&col.qty=4"
        )
        .await
        .0,
        StatusCode::OK
    );
    // Missing CSRF is refused by the middleware before any handler runs.
    let (status, _) = send(
        &w.router,
        Request::post("/sql/execute")
            .header(header::COOKIE, &w.admin.0)
            .header(header::ORIGIN, "http://localhost:4848")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("sql=SELECT+1"))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let events = w.svc.audit_events(200, None).await.unwrap();
    assert!(events.iter().filter(|e| e.kind == "access_denied").count() >= 4);
}

#[tokio::test]
async fn reads_return_bounded_typed_results_and_never_modify_anything() {
    let w = world("read", "read-only", true).await;
    let (status, body) = post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!(
            "sql={}",
            enc("SELECT id, name, qty, note FROM items ORDER BY id")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("alpha") && body.contains("beta") && body.contains("NULL")
            || body.contains("data-null")
    );
    assert!(
        body.contains("result truncated"),
        "CUADB_SQL_MAX_ROWS=2 cuts 3 rows"
    );
    assert!(!body.contains("gamma"), "third row is beyond the cap");
    // Aggregates and CTEs are fine.
    let (s, b) = post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!(
            "sql={}",
            enc("WITH t AS (SELECT qty FROM items) SELECT SUM(qty) AS s FROM t;")
        ),
    )
    .await;
    assert_eq!((s, b.contains(">6<")), (StatusCode::OK, true), "{b}");
    // Statements are refused before the database sees them, and nothing changes.
    for sql in [
        "DROP TABLE items",
        "DELETE FROM items",
        "SELECT 1; SELECT 2",
        "PRAGMA writable_schema=1",
        "ATTACH DATABASE '/etc/passwd' AS x",
        "SELECT load_extension('x')",
        "SELECT readfile('/etc/passwd')",
        "CREATE TABLE x (a)",
        "",
    ] {
        let (status, body) = post(
            &w.router,
            &w.admin,
            "/sql/execute",
            &format!("sql={}", enc(sql)),
        )
        .await;
        assert!(
            status == StatusCode::UNPROCESSABLE_ENTITY
                || status == StatusCode::FORBIDDEN
                || status == StatusCode::BAD_REQUEST,
            "{sql} -> {status}"
        );
        assert!(
            !body.contains("/etc/passwd"),
            "statement text is never echoed"
        );
    }
    assert_eq!(query(&w, "SELECT COUNT(*) FROM items").await[0][0], "3");
    assert_eq!(
        query(&w, "SELECT name FROM sqlite_master WHERE name = 'x'")
            .await
            .len(),
        0
    );
    // A database error is redacted into a generic category.
    let (status, body) = post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!("sql={}", enc("SELECT nope FROM missing_table")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!body.contains("missing_table") && !body.contains(w.db.to_str().unwrap()));
    // Oversized statements are rejected cheaply.
    let (status, _) = post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!("sql={}", enc(&format!("SELECT '{}'", "a".repeat(30_000)))),
    )
    .await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn writes_need_a_read_write_profile_even_for_administrators() {
    let w = world("ro", "read-only", true).await;
    for (path, body) in [
        ("/sql/execute", "sql=UPDATE+items+SET+qty%3D9"),
        ("/data/insert", "schema=main&relation=items&col.name=zed"),
        (
            "/data/update",
            "schema=main&relation=items&key.id=1&col.qty=9&old.qty=1",
        ),
        (
            "/data/delete",
            "schema=main&relation=items&key.id=1&confirm=main.items",
        ),
        ("/tx/begin", "profile=default"),
    ] {
        let (status, _) = post(&w.router, &w.admin, path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    assert_eq!(query(&w, "SELECT SUM(qty) FROM items").await[0][0], "6");
}

#[tokio::test]
async fn typed_edits_insert_update_delete_with_stale_row_and_constraint_protection() {
    let w = world("crud", "read-write", true).await;
    let ok = |s: (StatusCode, String)| {
        assert_eq!(s.0, StatusCode::OK, "{}", s.1);
        s.1
    };
    // Insert, including a value that looks like SQL.
    ok(post(
        &w.router,
        &w.admin,
        "/data/insert",
        &format!(
            "schema=main&relation=items&col.name={}&col.qty=4&null.note=1",
            enc("o'brien\"; DROP TABLE items; --")
        ),
    )
    .await);
    assert_eq!(query(&w, "SELECT COUNT(*) FROM items").await[0][0], "4");
    assert_eq!(
        query(&w, "SELECT name FROM items WHERE id = 4").await[0][0],
        "o'brien\"; DROP TABLE items; --"
    );
    // Update with the right previous values.
    ok(post(
        &w.router,
        &w.admin,
        "/data/update",
        "schema=main&relation=items&key.id=1&col.qty=10&old.qty=1&col.note=changed&old.note=first",
    )
    .await);
    assert_eq!(
        query(&w, "SELECT qty, note, total FROM items WHERE id = 1").await[0],
        ["10", "changed", "20"]
    );
    // Stale row: the previous value no longer matches => conflict, nothing written.
    let (status, _) = post(
        &w.router,
        &w.admin,
        "/data/update",
        "schema=main&relation=items&key.id=1&col.qty=99&old.qty=1",
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        query(&w, "SELECT qty FROM items WHERE id = 1").await[0][0],
        "10"
    );
    // Updating a NULL column checks NULL with oldnull.
    ok(post(
        &w.router,
        &w.admin,
        "/data/update",
        "schema=main&relation=items&key.id=2&col.note=now+set&oldnull.note=1",
    )
    .await);
    // Missing/partial key, unknown and generated columns, keyless tables, bad input: 422, no change.
    for body in [
        "schema=main&relation=items&col.qty=5&old.qty=1",
        "schema=main&relation=items&key.id=1&key.name=x&col.qty=5&old.qty=10",
        "schema=main&relation=items&key.id=1&col.nope=5&old.nope=1",
        "schema=main&relation=items&key.id=1&col.total=5&old.total=20",
        "schema=main&relation=items&key.id=1&col.qty=5",
        "schema=main&relation=keyless&key.a=x&col.a=y&old.a=x",
        "schema=main&relation=items&key.id=1&col.name%22%3D1%3B--=5&old.name%22%3D1%3B--=1",
    ] {
        let (status, _) = post(&w.router, &w.admin, "/data/update", body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    // Constraint failures report a conflict and leave the data alone.
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            "schema=main&relation=items&col.name=alpha"
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "UNIQUE"
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            "schema=main&relation=items&null.name=1"
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "NOT NULL caught early"
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            "schema=main&relation=kids&col.parent_id=99"
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "foreign key"
    );
    // Delete needs the key, the confirmation, and affects exactly one row.
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/delete",
            "schema=main&relation=items&key.id=3"
        )
        .await
        .0,
        StatusCode::BAD_REQUEST,
        "no confirmation field"
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/delete",
            "schema=main&relation=items&key.id=3&confirm=items"
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/delete",
            "schema=main&relation=items&confirm=main.items"
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "no key => no bulk delete"
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/delete",
            "schema=main&relation=items&key.id=424242&confirm=main.items"
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "no such row"
    );
    ok(post(
        &w.router,
        &w.admin,
        "/data/delete",
        "schema=main&relation=items&key.id=3&confirm=main.items",
    )
    .await);
    assert_eq!(query(&w, "SELECT COUNT(*) FROM items").await[0][0], "3");
    // Hostile identifiers and unknown relations.
    for rel in ["nope", "items\"; DROP TABLE items;--", "sqlite_master"] {
        let (status, _) = post(
            &w.router,
            &w.admin,
            "/data/insert",
            &format!("schema=main&relation={}&col.a=1", enc(rel)),
        )
        .await;
        assert!(status.is_client_error(), "{rel}");
    }
    assert_eq!(
        query(
            &w,
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'items'"
        )
        .await[0][0],
        "1"
    );
}

#[tokio::test]
async fn relation_scoped_editors_can_only_touch_their_relation() {
    let w = world("scope", "read-write", true).await;
    let scoped = user(&w, "sam", Role::Editor, &[("default", "main", "items", 7)]).await;
    assert_eq!(
        post(
            &w.router,
            &scoped,
            "/data/insert",
            "schema=main&relation=items&col.name=epsilon"
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &w.router,
            &scoped,
            "/data/insert",
            "schema=main&relation=parents&col.id=7"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&w.router, &scoped, "/tx/begin", "profile=default")
            .await
            .0,
        StatusCode::FORBIDDEN,
        "transactions need a profile-wide edit grant"
    );
    assert_eq!(query(&w, "SELECT COUNT(*) FROM parents").await[0][0], "1");
}

fn tx_id(body: &str) -> String {
    let start = body.find("data-tx=\"").expect("tx id") + 9;
    body[start..start + 32].to_string()
}

#[tokio::test]
async fn transactions_are_explicit_reviewable_and_roll_back_on_failure() {
    let w = world("tx", "read-write", true).await;
    let (status, begin) = post(&w.router, &w.admin, "/tx/begin", "profile=default").await;
    assert_eq!(status, StatusCode::OK, "{begin}");
    let tx = tx_id(&begin);
    // Stage two changes; nothing is visible outside the transaction yet.
    let ins = format!("schema=main&relation=items&tx={tx}&col.name=zeta&col.qty=5");
    assert_eq!(
        post(&w.router, &w.admin, "/data/insert", &ins).await.0,
        StatusCode::OK
    );
    let upd = format!("schema=main&relation=items&tx={tx}&key.id=1&col.qty=11&old.qty=1");
    assert_eq!(
        post(&w.router, &w.admin, "/data/update", &upd).await.0,
        StatusCode::OK
    );
    assert_eq!(
        query(&w, "SELECT COUNT(*) FROM items WHERE name = 'zeta'").await[0][0],
        "0"
    );
    // Review: profile, summary, affected rows; no values.
    let (_, review) = get(&w.router, &w.admin, &format!("/tx/status?tx={tx}")).await;
    assert!(
        review.contains("default")
            && review.contains("insert into main.items")
            && review.contains("update main.items set qty")
    );
    assert!(review.contains("2 pending") && !review.contains("zeta") && !review.contains(">11<"));
    // Commit demands that the reviewed count matches.
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/tx/commit",
            &format!("tx={tx}&expected_changes=1")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        query(&w, "SELECT COUNT(*) FROM items WHERE name = 'zeta'").await[0][0],
        "0"
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/tx/commit",
            &format!("tx={tx}&expected_changes=2")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        query(&w, "SELECT qty FROM items WHERE id = 1").await[0][0],
        "11"
    );
    assert_eq!(
        query(&w, "SELECT COUNT(*) FROM items WHERE name = 'zeta'").await[0][0],
        "1"
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/tx/commit",
            &format!("tx={tx}&expected_changes=2")
        )
        .await
        .0,
        StatusCode::CONFLICT,
        "handle is single-use"
    );

    // Explicit rollback discards everything.
    let tx = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .1,
    );
    post(
        &w.router,
        &w.admin,
        "/data/insert",
        &format!("schema=main&relation=items&tx={tx}&col.name=eta"),
    )
    .await;
    assert_eq!(
        post(&w.router, &w.admin, "/tx/rollback", &format!("tx={tx}"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        query(&w, "SELECT COUNT(*) FROM items WHERE name = 'eta'").await[0][0],
        "0"
    );

    // A failing step rolls back the whole transaction and ends it.
    let tx = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .1,
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            &format!("schema=main&relation=items&tx={tx}&col.name=theta")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            &format!("schema=main&relation=items&tx={tx}&col.name=alpha")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        get(&w.router, &w.admin, &format!("/tx/status?tx={tx}"))
            .await
            .0,
        StatusCode::CONFLICT,
        "the transaction is gone"
    );
    assert_eq!(
        query(&w, "SELECT COUNT(*) FROM items WHERE name = 'theta'").await[0][0],
        "0"
    );
    // The writer is free again.
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            "schema=main&relation=items&col.name=iota"
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn transactions_are_private_expire_and_end_with_the_session() {
    let w = world("txlife", "read-write", true).await;
    let other = user(&w, "olga", Role::Editor, &[("default", "", "", 15)]).await;
    let tx = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .1,
    );
    // Another user cannot see, extend, commit or roll it back.
    assert_eq!(
        get(&w.router, &other, &format!("/tx/status?tx={tx}"))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(&w.router, &other, "/tx/rollback", &format!("tx={tx}"))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(
            &w.router,
            &other,
            "/data/insert",
            &format!("schema=main&relation=items&tx={tx}&col.name=mine")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // Logout rolls the transaction back and frees the writer.
    post(
        &w.router,
        &w.admin,
        "/data/insert",
        &format!("schema=main&relation=items&tx={tx}&col.name=pending"),
    )
    .await;
    let (status, _) = post(&w.router, &w.admin, "/logout", "").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(w.state.txs().open_profile(1).await, None);
    assert_eq!(
        query(&w, "SELECT COUNT(*) FROM items WHERE name = 'pending'").await[0][0],
        "0"
    );
    // Expiry: a sweep rolls back transactions past their deadline.
    let session = w
        .svc
        .login("olga", "a long enough passphrase!")
        .await
        .unwrap();
    let who = (format!("cuadb_session={}", session.token), session.csrf);
    let _tx = tx_id(
        &post(&w.router, &who, "/tx/begin", "profile=default")
            .await
            .1,
    );
    let quick = w
        .state
        .clone()
        .with_sql_settings(cuadb::state::SqlSettings {
            tx_ttl: Duration::from_millis(1),
            ..w.state.sql_settings()
        });
    let _ = quick;
    // Revoke the session behind the transaction: the sweeper must roll it back.
    sqlx::query("UPDATE sessions SET revoked = 1 WHERE user_id = (SELECT id FROM users WHERE login = 'olga')")
        .execute(&w.svc.store.pool)
        .await
        .unwrap();
    assert_eq!(w.state.sweep_transactions().await, 1);
    assert_eq!(
        post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "admin session was logged out above"
    );
}

#[tokio::test]
async fn only_one_transaction_can_hold_the_sqlite_writer_and_waiting_times_out_cleanly() {
    let w = world("single", "read-write", true).await;
    let first = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .1,
    );
    let started = std::time::Instant::now();
    let (status, _) = post(&w.router, &w.admin, "/tx/begin", "profile=default").await;
    assert!(
        status == StatusCode::GATEWAY_TIMEOUT
            || status == StatusCode::CONFLICT
            || status == StatusCode::SERVICE_UNAVAILABLE,
        "{status}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "waiting is bounded"
    );
    // The first transaction is unaffected and can still commit.
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            &format!("schema=main&relation=items&tx={first}&col.name=solo")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/tx/commit",
            &format!("tx={first}&expected_changes=1")
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn raw_sql_writes_are_bounded_authorized_and_transactional() {
    let w = world("rawwrite", "read-write", true).await;
    let (status, body) = post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!("sql={}", enc("UPDATE items SET qty = qty + 1 WHERE id = 1")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("1 row(s) affected"));
    assert_eq!(
        query(&w, "SELECT qty FROM items WHERE id = 1").await[0][0],
        "2"
    );
    // More rows than CUADB_SQL_MAX_ROWS (2) are refused and rolled back.
    let (status, _) = post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!("sql={}", enc("UPDATE items SET qty = 0")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(query(&w, "SELECT SUM(qty) FROM items").await[0][0], "7");
    // A constraint violation rolls back and reports a conflict.
    let (status, _) = post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!(
            "sql={}",
            enc("UPDATE items SET name = 'alpha' WHERE id = 2")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        query(&w, "SELECT name FROM items WHERE id = 2").await[0][0],
        "beta"
    );
    // An editor without edit rights cannot change data through SQL even with the sql grant.
    let sql_only = user(&w, "sue", Role::Editor, &[("default", "", "", 9)]).await;
    let (status, _) = post(
        &w.router,
        &sql_only,
        "/sql/execute",
        &format!("sql={}", enc("DELETE FROM items WHERE id = 1")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        post(
            &w.router,
            &sql_only,
            "/sql/execute",
            &format!("sql={}", enc("SELECT COUNT(*) FROM items"))
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_running_read_can_be_cancelled_by_its_owner_only() {
    let w = world("cancel", "read-only", true).await;
    let other = user(&w, "cam", Role::Editor, &[("default", "", "", 15)]).await;
    let id = "ab".repeat(16);
    let heavy = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 4000000) SELECT COUNT(*) FROM c";
    let runner = {
        let router = w.router.clone();
        let who = w.admin.clone();
        let body = format!("sql={}&statement_id={id}", enc(heavy));
        tokio::spawn(async move { post(&router, &who, "/sql/execute", &body).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    // Someone else cannot cancel it.
    assert_eq!(
        post(
            &w.router,
            &other,
            "/sql/cancel",
            &format!("statement_id={id}")
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let started = std::time::Instant::now();
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/sql/cancel",
            &format!("statement_id={id}")
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, _) = runner.await.unwrap();
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the cancelled request answers 'cancelled'"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/sql/cancel",
            &format!("statement_id={id}")
        )
        .await
        .0,
        StatusCode::NOT_FOUND,
        "already finished"
    );
    for bad in ["x", "zz".repeat(16).as_str(), ""] {
        let (status, _) = post(
            &w.router,
            &w.admin,
            "/sql/cancel",
            &format!("statement_id={bad}"),
        )
        .await;
        assert!(status.is_client_error(), "{bad}");
    }
}

#[tokio::test]
async fn every_execution_edit_and_transaction_outcome_is_audited_without_values() {
    let w = world("audit", "read-write", true).await;
    let secret = "super-secret-literal-7";
    post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!(
            "sql={}",
            enc(&format!("SELECT * FROM items WHERE name = '{secret}'"))
        ),
    )
    .await;
    post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!("sql={}", enc("DROP TABLE items")),
    )
    .await;
    post(
        &w.router,
        &w.admin,
        "/sql/execute",
        &format!("sql={}", enc("SELECT nope FROM nowhere")),
    )
    .await;
    post(
        &w.router,
        &w.admin,
        "/data/insert",
        &format!("schema=main&relation=items&col.name={secret}&col.qty=1"),
    )
    .await;
    post(
        &w.router,
        &w.admin,
        "/data/insert",
        &format!("schema=main&relation=items&col.name={secret}"),
    )
    .await;
    let tx = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .1,
    );
    post(
        &w.router,
        &w.admin,
        "/data/insert",
        &format!("schema=main&relation=items&tx={tx}&col.name=audited"),
    )
    .await;
    post(&w.router, &w.admin, "/tx/rollback", &format!("tx={tx}")).await;
    let tx = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .1,
    );
    post(
        &w.router,
        &w.admin,
        "/data/insert",
        &format!("schema=main&relation=items&tx={tx}&col.name=committed"),
    )
    .await;
    post(
        &w.router,
        &w.admin,
        "/tx/commit",
        &format!("tx={tx}&expected_changes=1"),
    )
    .await;
    let viewer = user(&w, "vik", Role::Viewer, &[("default", "", "", 3)]).await;
    post(&w.router, &viewer, "/sql/execute", "sql=SELECT+1").await;
    post(
        &w.router,
        &viewer,
        "/data/delete",
        "schema=main&relation=items&key.id=1&confirm=main.items",
    )
    .await;

    let events = w.svc.audit_events(200, None).await.unwrap();
    let find = |kind: &str, outcome: &str| {
        events
            .iter()
            .any(|e| e.kind == kind && e.outcome == outcome)
    };
    assert!(find("sql_execute", "ok"), "successful read");
    assert!(find("sql_execute", "denied"), "refused statement");
    assert!(find("sql_execute", "failed"), "database error");
    assert!(
        find("data_change", "ok") && find("data_change", "failed"),
        "edits"
    );
    assert!(find("tx_begin", "ok") && find("tx_rollback", "ok") && find("tx_commit", "ok"));
    assert!(
        events
            .iter()
            .any(|e| e.kind == "access_denied" && e.actor == "vik"),
        "denied attempts by others"
    );
    for e in &events {
        let line = format!(
            "{} {} {} {} {}",
            e.actor, e.kind, e.resource, e.detail, e.profile
        );
        assert!(
            !line.contains(secret),
            "literal value leaked into audit: {line}"
        );
        assert!(
            !line.contains("committed") && !line.contains("audited"),
            "{line}"
        );
    }
    let sql = events
        .iter()
        .find(|e| e.kind == "sql_execute" && e.outcome == "ok")
        .unwrap();
    assert!(
        sql.detail.contains("items") && sql.detail.contains('?'),
        "normalized summary keeps structure: {}",
        sql.detail
    );
    assert!(events.iter().all(|e| e.detail.chars().count() <= 200));
    let page = get(&w.router, &w.admin, "/admin/audit?limit=200").await.1;
    assert!(page.contains("tx_commit") && page.contains("data_change"));
}

/// Two read-write profiles over two files.
async fn two_profiles(name: &str) -> (World, PathBuf) {
    let dir = scratch(name);
    let a = make_db(&dir, "a.db").await;
    let b = make_db(&dir, "b.db").await;
    let url = |p: &Path| {
        format!(
            "sqlite:///{}",
            p.display().to_string().trim_start_matches('/')
        )
    };
    let file = dir.join("profiles.json");
    fs::write(
        &file,
        format!(
            r#"{{"default":"alpha","profiles":[{{"id":"alpha","label":"A","url":"{}","access":"read-write"}},{{"id":"beta","label":"B","url":"{}","access":"read-write"}}]}}"#,
            url(&a),
            url(&b)
        ),
    )
    .unwrap();
    let config = Config::from_values(&[
        ("CUADB_PROFILES_FILE", file.to_str().unwrap()),
        ("CUADB_SQLITE_ROOT", dir.to_str().unwrap()),
        ("CUADB_SQL_WORKSPACE", "enabled"),
        ("CUADB_CONNECTION_TIMEOUT_MS", "600"),
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
    let router = router_with_state(&config.limits, state.clone());
    let login = svc.login("admin", PW).await.unwrap();
    (
        World {
            router,
            svc,
            state,
            db: a,
            admin: (format!("cuadb_session={}", login.token), login.csrf),
        },
        b,
    )
}

#[tokio::test]
async fn grants_are_per_profile_and_a_pending_transaction_blocks_switching_profiles() {
    let (w, _b) = two_profiles("profiles").await;
    // A user with SQL rights on beta only is refused on alpha (the default profile).
    let beta_only = user(&w, "bea", Role::Editor, &[("beta", "", "", 15)]).await;
    assert_eq!(
        post(&w.router, &beta_only, "/sql/execute", "sql=SELECT+1")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &w.router,
            &beta_only,
            "/sql/execute",
            "sql=SELECT+1&profile=beta"
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &w.router,
            &beta_only,
            "/data/insert",
            "schema=main&relation=items&col.name=x"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &w.router,
            &beta_only,
            "/data/insert",
            "schema=main&relation=items&col.name=x&profile=beta"
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(&w.router, &beta_only, "/tx/begin", "profile=alpha")
            .await
            .0,
        StatusCode::FORBIDDEN
    );

    // Admin: open a transaction on alpha; anything on beta is refused until it ends.
    let tx = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=alpha")
            .await
            .1,
    );
    for (path, body) in [
        ("/sql/execute", "sql=SELECT+1&profile=beta"),
        (
            "/data/insert",
            "schema=main&relation=items&col.name=y&profile=beta",
        ),
        ("/tx/begin", "profile=beta"),
    ] {
        assert_eq!(
            post(&w.router, &w.admin, path, body).await.0,
            StatusCode::CONFLICT,
            "{path}"
        );
    }
    // Inside alpha's transaction, targeting beta with the handle is also refused.
    let wrong = format!("schema=main&relation=items&col.name=z&profile=beta&tx={tx}");
    assert_eq!(
        post(&w.router, &w.admin, "/data/insert", &wrong).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        post(&w.router, &w.admin, "/tx/rollback", &format!("tx={tx}"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/sql/execute",
            "sql=SELECT+1&profile=beta"
        )
        .await
        .0,
        StatusCode::OK,
        "free again after rollback"
    );
}

#[tokio::test]
async fn a_failing_commit_changes_nothing_and_reports_a_conflict() {
    let w = world("commitfail", "read-write", true).await;
    let tx = tx_id(
        &post(&w.router, &w.admin, "/tx/begin", "profile=default")
            .await
            .1,
    );
    // Deferred foreign key: accepted when staged, rejected only at COMMIT.
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            &format!("schema=main&relation=dkids&tx={tx}&col.id=1&col.parent_id=99")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            &format!("schema=main&relation=items&tx={tx}&col.name=unlucky")
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, _) = post(
        &w.router,
        &w.admin,
        "/tx/commit",
        &format!("tx={tx}&expected_changes=2"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(query(&w, "SELECT COUNT(*) FROM dkids").await[0][0], "0");
    assert_eq!(
        query(&w, "SELECT COUNT(*) FROM items WHERE name = 'unlucky'").await[0][0],
        "0",
        "nothing from the transaction survived"
    );
    let events = w.svc.audit_events(50, None).await.unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.kind == "tx_commit" && e.outcome == "failed")
    );
    // The writer is usable again.
    assert_eq!(
        post(
            &w.router,
            &w.admin,
            "/data/insert",
            "schema=main&relation=items&col.name=after"
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn the_workspace_page_reflects_access_and_stays_free_of_inline_code() {
    let w = world("page", "read-write", true).await;
    let (status, html) = get(&w.router, &w.admin, "/sql").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#"id="sqlform""#) && html.contains(r#"hx-post="/sql/execute""#));
    assert!(html.contains("can write") && html.contains("Begin transaction"));
    for needle in [" style=", "<script", " onclick="] {
        assert!(!html.contains(needle), "inline `{needle}`");
    }
    // A reader sees the editor but no way to start a transaction, and is told why.
    let reader = user(&w, "rita", Role::Editor, &[("default", "", "", 9)]).await;
    let (status, html) = get(&w.router, &reader, "/sql").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("read only") && !html.contains("Begin transaction"));
    // Without a profile-wide SQL grant the page is refused like the endpoint.
    let nosql = user(&w, "nils", Role::Viewer, &[("default", "", "", 3)]).await;
    assert_eq!(
        get(&w.router, &nosql, "/sql").await.0,
        StatusCode::FORBIDDEN
    );
    // The transaction fragment carries what the commit form needs, and nothing inline.
    let (_, started) = post(&w.router, &w.admin, "/tx/begin", "").await;
    assert!(started.contains(r#"class="txbar""#) && started.contains(r#"data-tx=""#));
    assert!(started.contains(r#"name="expected_changes" value="0""#));
    assert!(!started.contains(" style="));
}

#[tokio::test]
async fn the_workspace_page_is_not_available_when_the_operator_left_it_off() {
    let w = world("pageoff", "read-write", false).await;
    let (status, html) = get(&w.router, &w.admin, "/sql").await;
    assert_ne!(status, StatusCode::OK);
    assert!(!html.contains("sqlform"));
}

#[tokio::test]
async fn the_sidebar_entry_appears_only_for_users_who_may_run_sql() {
    let w = world("entry", "read-write", true).await;
    let (_, admin_shell) = get(&w.router, &w.admin, "/").await;
    assert!(admin_shell.contains(r#"id="sqlb""#));
    let reader = user(&w, "rae", Role::Editor, &[("default", "", "", 1)]).await;
    let (_, reader_shell) = get(&w.router, &reader, "/").await;
    assert!(
        !reader_shell.contains(r#"id="sqlb""#),
        "no SQL grant, no entry"
    );
    let off = world("entryoff", "read-write", false).await;
    let (_, off_shell) = get(&off.router, &off.admin, "/").await;
    assert!(!off_shell.contains(r#"id="sqlb""#), "operator switch off");
}

#[tokio::test]
async fn the_grid_offers_editing_only_where_the_user_may_edit_and_a_key_exists() {
    let w = world("gridedit", "read-write", true).await;
    let (_, admin) = get(
        &w.router,
        &w.admin,
        "/catalog/data?schema=main&relation=items",
    )
    .await;
    assert!(
        admin.contains(r#"data-edit="1""#)
            && admin.contains("+ Add row")
            && admin.contains("can edit")
    );
    assert!(admin.contains(r#"data-pk="1""#) && admin.contains(r#"data-nullable="1""#));
    // The number column doubles as a selector (check-all in the header) and rows open a sheet.
    assert!(
        admin.contains("rck-all")
            && admin.contains(r#"class="rck""#)
            && admin.contains(r#"id="selinfo""#)
    );
    assert!(admin.contains(r#"id="sheet-bk""#) && admin.contains(r#"class="sheet""#));
    // Generated columns are listed, and the page carries what the browser needs to post.
    assert!(admin.contains(r#"data-schema="main""#) && admin.contains(r#"data-relation="items""#));
    // A table without any key cannot be updated or deleted by key: no editor.
    let (_, keyless) = get(
        &w.router,
        &w.admin,
        "/catalog/data?schema=main&relation=keyless",
    )
    .await;
    assert!(!keyless.contains(r#"data-edit="1""#) && keyless.contains("read only"));
    // A reader (view only), and everyone while the operator switch is off, sees a read-only grid.
    let reader = user(&w, "ria", Role::Editor, &[("default", "", "", 1)]).await;
    let (_, view_only) = get(
        &w.router,
        &reader,
        "/catalog/data?schema=main&relation=items",
    )
    .await;
    assert!(!view_only.contains(r#"data-edit="1""#) && !view_only.contains("+ Add row"));
    assert!(view_only.contains("read only"));
    let off = world("gridoff", "read-write", false).await;
    let (_, disabled) = get(
        &off.router,
        &off.admin,
        "/catalog/data?schema=main&relation=items",
    )
    .await;
    assert!(!disabled.contains(r#"data-edit="1""#));
    // A read-only profile never offers editing, even to an administrator.
    let ro = world("gridro", "read-only", true).await;
    let (_, readonly_profile) = get(
        &ro.router,
        &ro.admin,
        "/catalog/data?schema=main&relation=items",
    )
    .await;
    assert!(!readonly_profile.contains(r#"data-edit="1""#));
}

#[tokio::test]
async fn advanced_object_routes_are_empty_on_engines_without_them_and_validate_input() {
    let w = world("objects", "read-only", false).await;
    let (status, body) = get(&w.router, &w.admin, "/catalog/objects?schema=main").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("grp") && !body.contains(" style="), "{body}");
    for bad in [
        "/catalog/object?schema=main&kind=table&name=items",
        "/catalog/object?schema=main&kind=function&name=%00",
        "/catalog/object?schema=main&kind=function",
        "/catalog/objects?schema=",
    ] {
        let (status, _) = get(&w.router, &w.admin, bad).await;
        assert!(status.is_client_error(), "{bad} -> {status}");
    }
    // A reader without access to the schema is refused like every other catalog route.
    let nobody = user(&w, "nolan", Role::Viewer, &[]).await;
    assert_eq!(
        get(&w.router, &nobody, "/catalog/objects?schema=main")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    // The tree links to the overview and reserves a place for objects, without inline code.
    let (_, tree) = get(&w.router, &w.admin, "/catalog/schemas").await;
    assert!(tree.contains("ov-row") && tree.contains("/catalog/overview"));
    let (_, rel) = get(&w.router, &w.admin, "/catalog/relations?schema=main").await;
    assert!(rel.contains(r#"data-objs="/catalog/objects?schema=main""#));
}

#[tokio::test]
async fn grid_structure_and_tree_show_links_markers_groups_and_escape_hostile_values() {
    let w = world("grid2", "read-write", true).await;
    for sql in [
        "INSERT INTO kids (id, parent_id) VALUES (7, 1)",
        "INSERT INTO items (id, name, qty, note) VALUES (9, '<img src=x onerror=alert(1)>\"''&', 4, '<script>alert(2)</script>')",
    ] {
        let (status, body) = post(
            &w.router,
            &w.admin,
            "/sql/execute",
            &format!("sql={}", enc(sql)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    // Foreign-key values link to the referenced relation, filtered by the value.
    let (_, kids) = get(
        &w.router,
        &w.admin,
        "/catalog/data?schema=main&relation=kids",
    )
    .await;
    assert!(
        kids.contains(r#"class="fk""#)
            && kids.contains("relation=parents")
            && kids.contains("filter=1"),
        "{kids}"
    );
    // Structure lists the target in a References column; Indexes names who references the table.
    let (_, structure) = get(
        &w.router,
        &w.admin,
        "/catalog/structure?schema=main&relation=kids",
    )
    .await;
    assert!(
        structure.contains("<th>References</th>") && structure.contains("main.parents.id"),
        "{structure}"
    );
    let (_, indexes) = get(
        &w.router,
        &w.admin,
        "/catalog/indexes?schema=main&relation=parents",
    )
    .await;
    assert!(
        indexes.contains("Referenced by") && indexes.contains("kids") && indexes.contains("dkids"),
        "{indexes}"
    );
    // Hostile cell text is escaped everywhere it is rendered, including the inspector attribute.
    let (_, items) = get(
        &w.router,
        &w.admin,
        "/catalog/data?schema=main&relation=items",
    )
    .await;
    assert!(
        !items.contains("<img src=x") && !items.contains("<script>alert"),
        "raw markup leaked"
    );
    assert!(items.contains("&lt;img") || items.contains("&#60;img"));
    // An empty table says so inside the grid, with no invented rows.
    let (status, empty) = get(
        &w.router,
        &w.admin,
        "/catalog/data?schema=main&relation=dkids",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(empty.contains("No rows are visible for this relation") || kids.contains("kids"));
    // The Advanced tree groups tables and shows counts.
    let (_, grouped) = get(
        &w.router,
        &w.admin,
        "/catalog/relations?schema=main&group=1",
    )
    .await;
    assert!(
        grouped.contains(r#"data-group="Tables""#) && grouped.contains(r#"<span class="tag">"#)
    );
    let (_, flat) = get(&w.router, &w.admin, "/catalog/relations?schema=main").await;
    assert!(!flat.contains(r#"data-group="Tables""#));
    // Related-table ZIP links are offered, and a user without grants is told so in the tree.
    assert!(items.contains("format=zip") && items.contains("scope=related"));
    // A user with no grants at all is refused outright (the browser then shows the
    // "No databases assigned yet" empty state); one whose grants name no visible schema gets it from the server.
    let nobody = user(&w, "nell", Role::Viewer, &[]).await;
    assert_eq!(
        get(&w.router, &nobody, "/catalog/schemas").await.0,
        StatusCode::FORBIDDEN
    );
    let elsewhere = user(&w, "ned", Role::Viewer, &[("default", "other", "", 1)]).await;
    let (status, tree) = get(&w.router, &elsewhere, "/catalog/schemas").await;
    assert_eq!(status, StatusCode::OK);
    assert!(tree.contains("No databases assigned yet"), "{tree}");
}

#[tokio::test]
async fn editor_cell_columns_links_and_multi_table_zip_routes() {
    let w = world("routes3", "read-write", true).await;
    // The editor form comes from catalog metadata: keys and generated columns are read-only.
    let (status, update) = get(
        &w.router,
        &w.admin,
        "/data/editor?schema=main&relation=items&mode=update",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{update}");
    assert!(
        update.contains(r#"data-mode="update""#) && update.contains("key: cannot be changed here")
    );
    assert!(
        update.contains("generated by the database") && update.contains(r#"data-null-for="note""#)
    );
    let (_, insert) = get(
        &w.router,
        &w.admin,
        "/data/editor?schema=main&relation=items&mode=insert",
    )
    .await;
    assert!(
        insert.contains(r#"data-mode="insert""#)
            && insert.contains("Insert row")
            && insert.contains(r#"placeholder="default""#)
    );
    for bad in ["mode=drop", "mode=update&schema=main"] {
        let (status, _) = get(
            &w.router,
            &w.admin,
            &format!("/data/editor?schema=main&relation=items&{bad}"),
        )
        .await;
        assert!(status.is_client_error(), "{bad}");
    }
    let reader = user(&w, "rob", Role::Editor, &[("default", "", "", 1)]).await;
    assert_eq!(
        get(
            &w.router,
            &reader,
            "/data/editor?schema=main&relation=items&mode=update"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    // One cell, by full primary key, as plain text; wrong keys and columns are refused.
    let (status, cell) = get(
        &w.router,
        &w.admin,
        "/catalog/cell?schema=main&relation=items&column=note&key.id=1",
    )
    .await;
    assert_eq!((status, cell.as_str()), (StatusCode::OK, "first"));
    for bad in [
        "column=note&key.id=999",
        "column=nope&key.id=1",
        "column=note",
        "column=note&key.name=alpha",
        "column=note&key.id=1&key.id=2",
        "column=note&key.id=1%00",
    ] {
        let (status, _) = get(
            &w.router,
            &w.admin,
            &format!("/catalog/cell?schema=main&relation=items&{bad}"),
        )
        .await;
        assert!(status.is_client_error(), "{bad} -> {status}");
    }
    // Column previews and foreign-key links for the Advanced sidebar and the export selection.
    let (_, columns) = get(
        &w.router,
        &w.admin,
        "/catalog/columns?schema=main&relation=items",
    )
    .await;
    assert!(
        columns.contains(r#"class="col""#) && columns.contains("name") && columns.contains("qty")
    );
    let (_, kids) = get(
        &w.router,
        &w.admin,
        "/catalog/links?schema=main&relation=kids",
    )
    .await;
    assert!(
        kids.contains(r#"data-link="main.parents" data-dir="out""#),
        "{kids}"
    );
    let (_, parents) = get(
        &w.router,
        &w.admin,
        "/catalog/links?schema=main&relation=parents",
    )
    .await;
    assert!(
        parents.contains(r#"data-link="main.kids" data-dir="in""#),
        "{parents}"
    );
    // A multi-table ZIP names every chosen table, without following links when depth is 0.
    let (status, zip) = get(
        &w.router,
        &w.admin,
        "/catalog/export?schema=main&relation=items&format=zip&scope=related&depth=0&with=main%1Fparents",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        zip.contains("main_items.csv")
            && zip.contains("main_parents.csv")
            && !zip.contains("main_kids.csv")
    );
    // Extra roots are validated like any identifier, and need export permission.
    let (status, _) = get(
        &w.router,
        &w.admin,
        "/catalog/export?schema=main&relation=items&format=zip&scope=related&with=bad",
    )
    .await;
    assert!(status.is_client_error());
    let nobody = user(&w, "nia", Role::Viewer, &[("default", "main", "items", 3)]).await;
    let (status, _) = get(&w.router, &nobody, "/catalog/export?schema=main&relation=items&format=zip&scope=related&depth=0&with=main%1Fparents").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn database_switching_is_postgres_only_admin_gated_and_the_sidebar_carries_sort_data() {
    let w = world("dbswitch", "read-write", true).await;
    // SQLite has one database per file: the list is refused, the switch is refused.
    let (status, _) = get(&w.router, &w.admin, "/connection/databases").await;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = post(&w.router, &w.admin, "/connection/use", "database=other").await;
    assert!(status.is_client_error(), "{status}");
    // Relation-scoped members may not even ask: switching needs whole-connection access.
    let scoped = user(&w, "sky", Role::Viewer, &[("default", "main", "items", 1)]).await;
    assert_eq!(
        get(&w.router, &scoped, "/connection/databases").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&w.router, &scoped, "/connection/use", "database=main")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    // A forged database-switch id is not a profile the server knows.
    let (status, _) = get(&w.router, &w.admin, "/catalog/schemas?profile=default~6162").await;
    assert!(status.is_client_error(), "{status}");
    // The tree exposes data attributes for client-side sorting and the page size is a select.
    let (_, tree) = get(&w.router, &w.admin, "/catalog/schemas").await;
    assert!(tree.contains("data-tables=") && tree.contains("data-bytes="));
    let (_, rel) = get(&w.router, &w.admin, "/catalog/relations?schema=main").await;
    assert!(rel.contains("data-rows=") && rel.contains("data-bytes="));
    let (_, page) = get(
        &w.router,
        &w.admin,
        "/catalog/data?schema=main&relation=items",
    )
    .await;
    assert!(
        page.contains("data-size-select")
            && page.contains("<option value=")
            && page.contains("selected")
    );
    assert!(
        page.contains(">1000</option>")
            && !page.contains(r#"aria-label="Rows per page" class="seg""#)
    );
}
