//! Saved connections: encrypted at rest, admin-only, never shown back, restored on restart.

use std::{fs, path::PathBuf};

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
    secrets::{SecretBox, SecretKey},
    state::AppState,
};
use sqlx::Connection;
use tower::ServiceExt;

const PW: &str = "a long enough passphrase";
const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct World {
    router: Router,
    svc: AuthService,
    dir: PathBuf,
    admin: (String, String),
}

async fn sqlite_file(dir: &std::path::Path, name: &str) {
    let mut c =
        sqlx::SqliteConnection::connect(&format!("sqlite://{}?mode=rwc", dir.join(name).display()))
            .await
            .unwrap();
    sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
}

async fn world(name: &str, with_key: bool) -> World {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("conn-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    sqlite_file(&dir, "base.db").await;
    sqlite_file(&dir, "extra.db").await;
    let config = Config::from_values(&[
        (
            "CUADB_DATABASE_URL",
            format!(
                "sqlite:///{}",
                dir.join("base.db")
                    .display()
                    .to_string()
                    .trim_start_matches('/')
            )
            .as_str(),
        ),
        ("CUADB_SQLITE_ROOT", dir.to_str().unwrap()),
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
    let mut state = AppState::from_config(&config)
        .unwrap()
        .with_auth(svc.clone());
    if with_key {
        state = state.with_vault(SecretBox::new(&SecretKey::parse_hex(KEY).unwrap()));
    }
    let router = router_with_state(&config.limits, state);
    let login = svc.login("admin", PW).await.unwrap();
    World {
        router,
        svc,
        dir,
        admin: (format!("cuadb_session={}", login.token), login.csrf),
    }
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, String) {
    let mut request = request;
    request
        .headers_mut()
        .entry(header::HOST)
        .or_insert("localhost:4848".parse().unwrap());
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

async fn post(w: &World, who: &(String, String), path: &str, body: &str) -> (StatusCode, String) {
    send(
        &w.router,
        Request::post(path)
            .header(header::COOKIE, &who.0)
            .header(header::ORIGIN, "http://localhost:4848")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("{body}&csrf={}", who.1)))
            .unwrap(),
    )
    .await
}

async fn get(w: &World, who: &(String, String), path: &str) -> (StatusCode, String) {
    send(
        &w.router,
        Request::get(path)
            .header(header::COOKIE, &who.0)
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn an_admin_saves_tests_edits_and_removes_a_connection_without_secrets_leaking() {
    let w = world("crud", true).await;
    let (status, list) = get(&w, &w.admin, "/admin/connections").await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.contains("server configuration") && list.contains("New connection"));

    // Test before saving: succeeds for a real file, fails (without echoing) for a bad one.
    let (status, ok) = post(
        &w,
        &w.admin,
        "/admin/connections/test",
        "engine=sqlite&file=extra.db",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ok}");
    assert!(ok.contains("Connected in"));
    let (status, bad) = post(
        &w,
        &w.admin,
        "/admin/connections/test",
        "engine=sqlite&file=..%2Fescape.db",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!bad.contains("escape"));

    // Save.
    let (status, saved) = post(
        &w,
        &w.admin,
        "/admin/connections",
        "label=Extra+DB&engine=sqlite&file=extra.db&writable=on",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert!(saved.contains("Connection saved"));
    let (_, profiles) = get(&w, &w.admin, "/connection/profiles").await;
    assert!(profiles.contains("Extra DB") && profiles.contains(r#"data-profile="extra-db""#));
    let (_, tree) = get(
        &w,
        &w.admin,
        "/catalog/relations?schema=main&profile=extra-db",
    )
    .await;
    assert!(
        tree.contains(">t<") || tree.contains("data-relation-name=\"t\""),
        "{tree}"
    );

    // The stored value is ciphertext: no path, no engine text.
    let row = w.svc.store.connection("extra-db").await.unwrap().unwrap();
    let blob = String::from_utf8_lossy(&row.url_enc).to_string();
    assert!(!blob.contains("extra.db") && !blob.contains("sqlite"));
    let vault = SecretBox::new(&SecretKey::parse_hex(KEY).unwrap());
    let url = String::from_utf8(vault.open("extra-db", &row.url_enc).unwrap()).unwrap();
    assert!(url.starts_with("sqlite:///") && url.ends_with("extra.db"));
    assert!(
        vault.open("other-id", &row.url_enc).is_none(),
        "bound to its id"
    );

    // The list and the edit form never contain the path of an encrypted URL's password part.
    let (_, form) = get(&w, &w.admin, "/admin/connections/form?id=extra-db").await;
    assert!(form.contains(r#"value="extra.db""#) && form.contains("Save changes"));
    let (_, list) = get(&w, &w.admin, "/admin/connections").await;
    assert!(list.contains("Extra DB") && list.contains("saved here"));

    // Duplicate name refused; edit works; the audit log names the change without the URL.
    let (status, dup) = post(
        &w,
        &w.admin,
        "/admin/connections",
        "label=Extra+DB&engine=sqlite&file=extra.db",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(dup.contains("already exists"));
    let (status, _) = post(
        &w,
        &w.admin,
        "/admin/connections/update",
        "id=extra-db&label=Extras&engine=sqlite&file=extra.db",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        get(&w, &w.admin, "/connection/profiles")
            .await
            .1
            .contains("Extras")
    );
    let (_, audit) = get(&w, &w.admin, "/admin/audit").await;
    assert!(audit.contains("connection_changed") && !audit.contains("extra.db"));

    // Remove: grants for it go too; file-defined connections can never be removed.
    let (status, _) = post(&w, &w.admin, "/admin/connections/delete", "id=default").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, gone) = post(&w, &w.admin, "/admin/connections/delete", "id=extra-db").await;
    assert_eq!(status, StatusCode::OK, "{gone}");
    assert!(
        !get(&w, &w.admin, "/connection/profiles")
            .await
            .1
            .contains("Extras")
    );
    assert!(w.svc.store.connection("extra-db").await.unwrap().is_none());
    let _ = fs::remove_dir_all(&w.dir);
}

#[tokio::test]
async fn saving_needs_the_key_and_an_administrator() {
    let w = world("nokey", false).await;
    let (status, refused) = post(
        &w,
        &w.admin,
        "/admin/connections",
        "label=X&engine=sqlite&file=extra.db",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(refused.contains("CUADB_SECRET_KEY") && !refused.contains("extra.db"));
    let (_, list) = get(&w, &w.admin, "/admin/connections").await;
    assert!(list.contains("openssl rand -hex 32") && !list.contains("+ New connection"));

    let w = world("viewer", true).await;
    let admin = w.svc.login("admin", PW).await.unwrap().principal;
    w.svc
        .create_user(&admin, "vic", "a long enough passphrase!", Role::Viewer)
        .await
        .unwrap();
    let s = w
        .svc
        .login("vic", "a long enough passphrase!")
        .await
        .unwrap();
    let vic = (format!("cuadb_session={}", s.token), s.csrf);
    assert_eq!(
        get(&w, &vic, "/admin/connections").await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &w,
            &vic,
            "/admin/connections",
            "label=X&engine=sqlite&file=extra.db"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(
            &w,
            &vic,
            "/admin/connections/test",
            "engine=sqlite&file=extra.db"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let _ = fs::remove_dir_all(&w.dir);
}

#[tokio::test]
async fn saved_connections_come_back_after_a_restart_and_need_the_same_key() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("conn-restart-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    sqlite_file(&dir, "base.db").await;
    sqlite_file(&dir, "extra.db").await;
    let data = dir.join("state");
    let url = format!(
        "sqlite:///{}",
        dir.join("base.db")
            .display()
            .to_string()
            .trim_start_matches('/')
    );
    let values = |key: Option<&'static str>| {
        let mut v: Vec<(&str, String)> = vec![
            ("CUADB_DATABASE_URL", url.clone()),
            ("CUADB_SQLITE_ROOT", dir.display().to_string()),
            ("CUADB_DATA_DIR", data.display().to_string()),
        ];
        if let Some(k) = key {
            v.push(("CUADB_SECRET_KEY", k.to_string()));
        }
        v
    };
    let cfg = |key| {
        let owned = values(key);
        let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (*a, b.as_str())).collect();
        Config::from_values(&refs).unwrap()
    };
    let with_key = cfg(Some(KEY));
    // First run: create the store, an admin and one saved connection.
    let store = AuthStore::open(&data, true).await.unwrap();
    bootstrap_admin(&store, "admin", PW).await.unwrap();
    let vault = SecretBox::new(&SecretKey::parse_hex(KEY).unwrap());
    let extra_url = format!(
        "sqlite:///{}",
        dir.join("extra.db")
            .display()
            .to_string()
            .trim_start_matches('/')
    );
    store
        .save_connection(&cuadb::auth::store::ConnectionRow {
            id: "extra".into(),
            label: "Extra".into(),
            engine: "sqlite".into(),
            url_enc: vault.seal("extra", extra_url.as_bytes()).unwrap(),
            writable: false,
        })
        .await
        .unwrap();
    store.close().await;
    // Restart with the key: it is registered and marked as saved here.
    let state = AppState::from_config(&with_key)
        .unwrap()
        .with_auth_from_config(&with_key)
        .await
        .unwrap();
    let infos = state.profiles();
    assert!(infos.iter().any(|p| p.id == "extra" && p.managed));
    assert!(infos.iter().any(|p| p.id == "default" && !p.managed));
    state.close().await;
    // Restart without the key, or with another one: it refuses to run, naming no secret.
    for key in [
        None,
        Some("fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"),
    ] {
        let config = cfg(key);
        let error = AppState::from_config(&config)
            .unwrap()
            .with_auth_from_config(&config)
            .await
            .err()
            .expect("must refuse");
        let message = error.to_string();
        assert!(
            message.contains("CUADB_SECRET_KEY") && !message.contains("extra.db"),
            "{message}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn the_test_endpoint_reaches_a_real_postgres_fixture_and_reports_only_a_category() {
    // Returns early (not evidence) unless a disposable cuadb_fixture_* database is named.
    let Ok(url) = std::env::var("CUADB_TEST_CATALOG_URL") else {
        return;
    };
    let parsed = url::Url::parse(&url).unwrap();
    assert!(
        parsed
            .path()
            .trim_start_matches('/')
            .starts_with("cuadb_fixture_")
    );
    let w = world("pgtest", true).await;
    let form = format!(
        "engine=postgresql&host={}&port={}&user={}&database={}&tls=disable",
        parsed.host_str().unwrap(),
        parsed.port().unwrap_or(5432),
        parsed.username(),
        parsed.path().trim_start_matches('/')
    );
    let (status, ok) = post(&w, &w.admin, "/admin/connections/test", &form).await;
    assert_eq!(status, StatusCode::OK, "{ok}");
    assert!(ok.contains("Connected in"));
    // Wrong database: a category, never the host, user or driver text.
    let wrong = form.replace(
        parsed.path().trim_start_matches('/'),
        "cuadb_fixture_does_not_exist",
    );
    let (status, bad) = post(&w, &w.admin, "/admin/connections/test", &wrong).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        bad.contains("Could not connect")
            && !bad.contains(parsed.username())
            && !bad.contains("127.0.0.1")
    );
    let _ = fs::remove_dir_all(&w.dir);
}
