//! SQLite adapter checks against throw-away database files created under
//! CARGO_TARGET_TMPDIR. These need no external service and always run.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{
    app::router_with_state,
    config::Config,
    db::{
        adapter::DatabaseAdapter,
        sqlite::{SqliteManager, SqliteUrlError, resolve_path},
        types::*,
    },
    state::AppState,
};
use sqlx::{Connection, SqliteConnection};
use tower::ServiceExt;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("sqlite-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn make_db(path: &Path, statements: &[&str]) {
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let mut conn = SqliteConnection::connect(&url).await.unwrap();
    for statement in statements {
        sqlx::query(statement).execute(&mut conn).await.unwrap();
    }
    conn.close().await.unwrap();
}

fn url(path: &Path) -> String {
    format!(
        "sqlite:///{}",
        path.display().to_string().trim_start_matches('/')
    )
}

fn open(path: &Path, root: &Path) -> SqliteManager {
    let config = Config::from_values(&[]).unwrap();
    SqliteManager::new(&url(path), root, &config.limits).unwrap()
}

fn router(manager: &SqliteManager) -> Router {
    let config = Config::from_values(&[]).unwrap();
    router_with_state(
        &config.limits,
        AppState::with_adapter(Arc::new(manager.catalog())),
    )
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

fn rel(name: &str) -> RelationId {
    RelationId {
        namespace: "main".into(),
        name: name.into(),
    }
}

#[test]
fn path_rules_reject_traversal_missing_directories_queries_and_foreign_schemes() {
    let root = scratch("paths");
    let inside = root.join("ok.db");
    fs::write(&inside, b"").unwrap();
    assert!(resolve_path(&url(&inside), &root).is_ok());
    let traversal = format!(
        "sqlite:///{}/sub/../../etc/passwd",
        root.display().to_string().trim_start_matches('/')
    );
    assert!(resolve_path(&traversal, &root).is_err());
    assert_eq!(
        resolve_path("sqlite:////etc/passwd", &root),
        Err(SqliteUrlError::Root)
    );
    assert_eq!(
        resolve_path(&url(&root.join("missing.db")), &root),
        Err(SqliteUrlError::Path)
    );
    assert_eq!(
        resolve_path(&url(&root), &root),
        Err(SqliteUrlError::Path),
        "directories are not databases"
    );
    for bad in [
        format!("{}?mode=rwc", url(&inside)),
        format!("{}#frag", url(&inside)),
        "sqlite::memory:".into(),
        "file:ok.db".into(),
        "postgres:///x".into(),
    ] {
        assert!(resolve_path(&bad, &root).is_err(), "{bad}");
    }
    // Error text never reveals the filesystem layout.
    let message = resolve_path(&url(&root.join("missing.db")), &root)
        .unwrap_err()
        .to_string();
    assert!(!message.contains(env!("CARGO_TARGET_TMPDIR")) && !message.contains("missing.db"));
}

#[cfg(unix)]
#[test]
fn symlinks_are_followed_only_when_the_target_stays_inside_the_root() {
    use std::os::unix::fs::symlink;
    let root = scratch("links");
    let outside = scratch("links-outside").join("secret.db");
    fs::write(&outside, b"").unwrap();
    let inside = root.join("real.db");
    fs::write(&inside, b"").unwrap();
    symlink(&outside, root.join("escape.db")).unwrap();
    symlink(&inside, root.join("alias.db")).unwrap();
    symlink(root.join("loop.db"), root.join("loop.db")).unwrap();
    assert_eq!(
        resolve_path(&url(&root.join("escape.db")), &root),
        Err(SqliteUrlError::Root)
    );
    assert_eq!(
        resolve_path(&url(&root.join("alias.db")), &root),
        Ok(inside.canonicalize().unwrap())
    );
    assert!(resolve_path(&url(&root.join("loop.db")), &root).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_files_report_unavailable_without_paths() {
    use std::os::unix::fs::PermissionsExt;
    let root = scratch("perm");
    let db = root.join("locked.db");
    make_db(&db, &["CREATE TABLE t(id INTEGER)"]).await;
    fs::set_permissions(&db, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::File::open(&db).is_ok() {
        eprintln!("running as a user that ignores file modes (root); skipping");
    } else {
        let manager = open(&db, &root);
        let snapshot = manager.snapshot().await;
        assert_ne!(snapshot.status, ConnectionStatus::Ready);
        assert!(!snapshot.detail(None).contains("locked.db"));
        manager.close().await;
    }
    fs::set_permissions(&db, fs::Permissions::from_mode(0o644)).unwrap();
}

#[tokio::test]
async fn browsing_never_modifies_the_file_or_creates_side_files() {
    let root = scratch("readonly");
    let db = root.join("data.db");
    make_db(
        &db,
        &[
            "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)",
            "INSERT INTO t(v) VALUES ('a'),('b')",
        ],
    )
    .await;
    let before = (
        fs::read(&db).unwrap(),
        fs::metadata(&db).unwrap().modified().unwrap(),
    );
    let manager = open(&db, &root);
    let router = router(&manager);
    for path in [
        "/catalog/relations?schema=main",
        "/catalog/data?schema=main&relation=t&size=1&sort=v&direction=desc",
        "/catalog/structure?schema=main&relation=t",
        "/catalog/indexes?schema=main&relation=t",
        "/catalog/export?schema=main&relation=t&format=csv&scope=relation",
    ] {
        let (status, _) = fetch(&router, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
    let write = sqlx::query("INSERT INTO t(v) VALUES ('x')")
        .execute(&sqlx_pool(&db).await)
        .await;
    drop(write);
    manager.close().await;
    let after = (
        fs::read(&db).unwrap(),
        fs::metadata(&db).unwrap().modified().unwrap(),
    );
    assert_eq!(before, after, "database bytes and mtime must be unchanged");
    let mut names: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["data.db"], "no -journal/-wal/-shm side files");
}

async fn sqlx_pool(db: &Path) -> sqlx::SqlitePool {
    // A fresh read-only pool, as the adapter would use: writes must fail.
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(db)
        .read_only(true)
        .create_if_missing(false);
    sqlx::SqlitePool::connect_with(options).await.unwrap()
}

#[tokio::test]
async fn catalog_distinguishes_tables_views_virtual_tables_and_hides_shadow_tables() {
    let root = scratch("kinds");
    let db = root.join("kinds.db");
    make_db(
        &db,
        &[
            "CREATE TABLE plain(id INTEGER PRIMARY KEY, name TEXT)",
            "CREATE VIEW plain_v AS SELECT id FROM plain",
            "CREATE VIRTUAL TABLE docs USING fts5(body)",
            "CREATE TABLE nr(a TEXT PRIMARY KEY, b INTEGER) WITHOUT ROWID",
            "CREATE TABLE gen(id INTEGER PRIMARY KEY, a INTEGER, v INTEGER GENERATED ALWAYS AS (a*2) VIRTUAL, s INTEGER GENERATED ALWAYS AS (a*3) STORED)",
            "INSERT INTO gen(a) VALUES (1),(2)",
        ],
    )
    .await;
    let manager = open(&db, &root);
    let catalog = manager.catalog();
    let list = catalog.list_relations("main").await.unwrap();
    let kind = |name: &str| {
        list.items
            .iter()
            .find(|r| r.id.name == name)
            .map(|r| r.kind)
    };
    assert_eq!(kind("plain"), Some(RelationKind::Table));
    assert_eq!(kind("plain_v"), Some(RelationKind::View));
    assert_eq!(kind("docs"), Some(RelationKind::VirtualTable));
    assert_eq!(kind("nr"), Some(RelationKind::Table));
    assert!(
        kind("docs_data").is_none() && kind("docs_idx").is_none(),
        "FTS shadow tables stay hidden"
    );
    assert!(list.items.iter().all(|r| !r.id.name.starts_with("sqlite_")));
    // Generated columns are listed with their kind, not silently dropped.
    let cols = catalog.describe_columns(&rel("gen")).await.unwrap();
    let generated: Vec<_> = cols
        .items
        .iter()
        .map(|c| (c.column.name.as_str(), c.generated.as_deref()))
        .collect();
    assert_eq!(
        generated,
        [
            ("id", None),
            ("a", None),
            ("v", Some("VIRTUAL")),
            ("s", Some("STORED"))
        ]
    );
    let page = catalog
        .fetch_rows(&rel("gen"), RowOptions::default())
        .await
        .unwrap();
    assert!(
        matches!(page.rows[1][2], Cell::Integer(4)) && matches!(page.rows[1][3], Cell::Integer(6))
    );
    // WITHOUT ROWID: ordered by its primary key, never by a missing rowid.
    let nr = catalog
        .fetch_rows(&rel("nr"), RowOptions::default())
        .await
        .unwrap();
    assert!(nr.stable_order && nr.rows.is_empty());
    // Views and virtual tables have no key: honestly report an unstable order.
    let view = catalog
        .fetch_rows(&rel("plain_v"), RowOptions::default())
        .await
        .unwrap();
    assert!(!view.stable_order);
    let stats = catalog.relation_statistics(&rel("docs")).await.unwrap();
    assert_eq!(
        stats.relation_kind,
        Availability::Available(RelationKind::VirtualTable)
    );
    assert!(matches!(stats.estimated_rows, Availability::Unavailable));
    assert!(catalog.describe_columns(&rel("docs_data")).await.is_err());
    manager.close().await;
}

#[tokio::test]
async fn unusual_names_and_empty_databases_are_listed_and_quoted_safely() {
    let root = scratch("names");
    let empty = root.join("empty.db");
    fs::write(&empty, b"").unwrap();
    let emptied = open(&empty, &root);
    assert_eq!(emptied.snapshot().await.status, ConnectionStatus::Ready);
    let list = emptied.catalog().list_relations("main").await.unwrap();
    assert!(list.items.is_empty() && !list.truncated);
    emptied.close().await;

    let db = root.join("names.db");
    make_db(
        &db,
        &[
            "CREATE TABLE \"select\"(\"from\" TEXT PRIMARY KEY, \"a\"\"b\" TEXT)",
            "CREATE TABLE \"my table; DROP TABLE x\"(id INTEGER)",
            "CREATE TABLE \"ünï 日本 😀\"(\"col umn\" INTEGER)",
            "INSERT INTO \"select\" VALUES ('k','<script>alert(1)</script>')",
            "INSERT INTO \"ünï 日本 😀\" VALUES (7)",
        ],
    )
    .await;
    let manager = open(&db, &root);
    let router = router(&manager);
    let (_, tree) = fetch(&router, "/catalog/relations?schema=main").await;
    assert!(
        tree.contains("select")
            && tree.contains("ünï 日本 😀")
            && tree.contains("my table; DROP TABLE x")
    );
    let (status, grid) = fetch(
        &router,
        "/catalog/data?schema=main&relation=select&sort=a%22b&direction=asc",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!grid.contains("<script>alert(1)"));
    let (status, grid) = fetch(
        &router,
        "/catalog/data?schema=main&relation=%C3%BCn%C3%AF%20%E6%97%A5%E6%9C%AC%20%F0%9F%98%80",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(grid.contains(">7<") || grid.contains("7</button>"));
    for bad in ["x%22%20OR%201=1", "sqlite_master", "nope"] {
        let (status, _) = fetch(
            &router,
            &format!("/catalog/data?schema=main&relation={bad}"),
        )
        .await;
        assert!(status.is_client_error(), "{bad}");
    }
    let (status, _) = fetch(&router, "/catalog/relations?schema=temp").await;
    assert!(
        status.is_client_error(),
        "only the main schema exists; no ATTACH/temp access"
    );
    manager.close().await;
}

#[tokio::test]
async fn dynamic_types_blobs_and_ordering_are_reported_honestly() {
    let root = scratch("dynamic");
    let db = root.join("dyn.db");
    make_db(
        &db,
        &[
            "CREATE TABLE mixed(id INTEGER PRIMARY KEY, anything, n INTEGER, t TEXT, r REAL, b BLOB)",
            "INSERT INTO mixed VALUES (1, 42, 7, 'seven', 1.5, x'00ff')",
            "INSERT INTO mixed VALUES (2, 'text', 'not-a-number', 99, 2, 'abc')",
            "INSERT INTO mixed VALUES (3, 3.25, NULL, NULL, NULL, NULL)",
            "INSERT INTO mixed VALUES (4, x'cafe', 8, 'x', 0.5, zeroblob(1048576))",
            "CREATE TABLE heap(a TEXT)",
            "INSERT INTO heap VALUES ('z'),('y'),('x')",
        ],
    )
    .await;
    let manager = open(&db, &root);
    let catalog = manager.catalog();
    let page = catalog
        .fetch_rows(&rel("mixed"), RowOptions::default())
        .await
        .unwrap();
    assert!(page.stable_order);
    assert!(matches!(page.rows[0][1], Cell::Integer(42)));
    assert!(matches!(page.rows[1][1], Cell::Text(ref s) if s == "text"));
    assert!(matches!(page.rows[2][1], Cell::Decimal(ref s) if s == "3.25"));
    assert!(matches!(page.rows[2][2], Cell::Null));
    assert!(matches!(page.rows[3][1], Cell::Bytes(ref b) if b == &[0xca, 0xfe]));
    // TEXT affinity turns 99 into text (plain label); INTEGER holding text is labelled as such.
    assert_eq!(page.previews[1][3].type_name, "TEXT");
    assert!(
        matches!(page.rows[1][2], Cell::Text(_))
            && page.previews[1][2].type_name == "INTEGER · text"
    );
    assert_eq!(page.previews[0][1].type_name, "dynamic · integer");
    // A 1 MiB blob is never loaded: only a bounded prefix and its true length are reported.
    assert!(matches!(page.rows[3][5], Cell::Truncated { ref preview, .. } if preview.len() < 1024));
    assert!(page.previews[3][5].text.starts_with("1048576 bytes"));
    assert!(page.previews[3][5].truncated);
    // Sorting: NULLs last in both directions; blobs are not sortable.
    for dir in [SortDirection::Asc, SortDirection::Desc] {
        let sorted = catalog
            .fetch_rows(
                &rel("mixed"),
                RowOptions {
                    sort: Some(SortSpec {
                        column: "r".into(),
                        direction: dir,
                    }),
                    ..RowOptions::default()
                },
            )
            .await
            .unwrap();
        assert!(
            matches!(sorted.rows[3][4], Cell::Null),
            "{dir:?}: NULL must be last"
        );
    }
    assert!(
        catalog
            .fetch_rows(
                &rel("mixed"),
                RowOptions {
                    sort: Some(SortSpec {
                        column: "b".into(),
                        direction: SortDirection::Asc
                    }),
                    ..RowOptions::default()
                }
            )
            .await
            .is_err()
    );
    // A rowid table without a primary key still has a deterministic (rowid) order.
    let heap = catalog
        .fetch_rows(
            &rel("heap"),
            RowOptions {
                limit: 2,
                ..RowOptions::default()
            },
        )
        .await
        .unwrap();
    assert!(heap.stable_order && heap.has_more);
    assert!(matches!(heap.rows[0][0], Cell::Text(ref s) if s == "z"));
    // Through HTTP the page stays small even with the 1 MiB blob.
    let router = router(&manager);
    let (_, html) = fetch(&router, "/catalog/data?schema=main&relation=mixed").await;
    assert!(html.len() < 60_000, "page was {} bytes", html.len());
    manager.close().await;
}

#[tokio::test]
async fn row_filter_is_a_literal_substring() {
    let root = scratch("filter");
    let db = root.join("f.db");
    make_db(
        &db,
        &[
            "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)",
            "INSERT INTO t(v) VALUES ('50%_off'),('500'),('a\\b')",
        ],
    )
    .await;
    let manager = open(&db, &root);
    let catalog = manager.catalog();
    for (needle, expected) in [
        ("%", 1),
        ("_", 1),
        ("50%", 1),
        ("\\", 1),
        ("50", 2),
        ("zzz", 0),
    ] {
        let page = catalog
            .fetch_rows(
                &rel("t"),
                RowOptions {
                    filter: Some(needle.into()),
                    ..RowOptions::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(page.rows.len(), expected, "filter {needle:?}");
    }
    manager.close().await;
}

/// Emulates a Docker `:ro` bind mount: read-only directory and read-only file.
#[cfg(unix)]
fn make_read_only(dir: &Path, db: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(db, fs::Permissions::from_mode(0o444)).unwrap();
    fs::set_permissions(dir, fs::Permissions::from_mode(0o555)).unwrap();
}

#[cfg(unix)]
fn restore(dir: &Path, db: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(db, fs::Permissions::from_mode(0o644)).unwrap();
}

#[cfg(unix)]
fn is_unrestricted(dir: &Path) -> bool {
    // root (or CAP_DAC_OVERRIDE) can write to a 0o555 directory; the mount test is meaningless.
    fs::File::create(dir.join(".probe"))
        .map(|_| fs::remove_file(dir.join(".probe")).is_ok())
        .unwrap_or(false)
}

#[cfg(unix)]
#[tokio::test]
async fn read_only_mount_with_a_rollback_journal_database_browses_cleanly() {
    let root = scratch("romount");
    let db = root.join("ro.db");
    make_db(
        &db,
        &[
            "PRAGMA journal_mode=DELETE",
            "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)",
            "INSERT INTO t(v) VALUES ('a')",
        ],
    )
    .await;
    make_read_only(&root, &db);
    if is_unrestricted(&root) {
        restore(&root, &db);
        return;
    }
    let before = fs::read(&db).unwrap();
    let manager = open(&db, &root);
    let router = router(&manager);
    let (status, html) = fetch(&router, "/catalog/data?schema=main&relation=t").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("1–1"));
    manager.close().await;
    assert_eq!(fs::read(&db).unwrap(), before);
    assert_eq!(
        fs::read_dir(&root).unwrap().count(),
        1,
        "no side files in the read-only mount"
    );
    restore(&root, &db);
}

#[cfg(unix)]
#[tokio::test]
async fn wal_databases_on_a_read_only_mount_fail_closed_and_are_documented() {
    let root = scratch("wal");
    let db = root.join("wal.db");
    make_db(
        &db,
        &[
            "PRAGMA journal_mode=WAL",
            "CREATE TABLE t(id INTEGER PRIMARY KEY)",
            "INSERT INTO t DEFAULT VALUES",
        ],
    )
    .await;
    // Cleanly closed WAL databases checkpoint and delete -wal; -shm may remain or not.
    for leftover in ["wal.db-wal", "wal.db-shm"] {
        let _ = fs::remove_file(root.join(leftover));
    }
    make_read_only(&root, &db);
    if is_unrestricted(&root) {
        restore(&root, &db);
        return;
    }
    let manager = open(&db, &root);
    let status = manager.snapshot().await.status;
    // Either outcome is acceptable; what matters is that nothing is written and no path leaks.
    eprintln!("WAL on read-only mount -> {status:?}");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    manager.close().await;
    restore(&root, &db);
}
