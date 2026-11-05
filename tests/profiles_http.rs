//! Trusted connection profiles end to end: file config -> AppState -> routes.
//! Uses real SQLite databases (no external service) plus an unreachable
//! PostgreSQL profile to prove failures stay isolated per profile.

use std::{
    fs,
    path::{Path, PathBuf},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use cuadb::{app::router_with_state, config::Config, state::AppState};
use sqlx::Connection;
use tower::ServiceExt;

struct Fixture {
    root: PathBuf,
    profiles: PathBuf,
}

async fn db(path: &Path, table: &str, values: &[&str]) {
    let mut conn =
        sqlx::SqliteConnection::connect(&format!("sqlite://{}?mode=rwc", path.display()))
            .await
            .unwrap();
    sqlx::query(&format!(
        "CREATE TABLE {table}(id INTEGER PRIMARY KEY, v TEXT)"
    ))
    .execute(&mut conn)
    .await
    .unwrap();
    for v in values {
        sqlx::query(&format!("INSERT INTO {table}(v) VALUES ('{v}')"))
            .execute(&mut conn)
            .await
            .unwrap();
    }
    conn.close().await.unwrap();
}

fn sqlite_url(path: &Path) -> String {
    format!(
        "sqlite:///{}",
        path.display().to_string().trim_start_matches('/')
    )
}

async fn fixture(name: &str) -> Fixture {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("profiles-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    db(
        &root.join("alpha.db"),
        "alpha_items",
        &["alpha-1", "alpha-2"],
    )
    .await;
    db(&root.join("beta.db"), "beta_items", &["beta-1"]).await;
    let profiles = root.join("profiles.json");
    let json = format!(
        r#"{{"default":"alpha","profiles":[
            {{"id":"alpha","label":"Alpha (SQLite)","url":"{}"}},
            {{"id":"beta","label":"Beta <b>(SQLite)</b>","url":"{}"}},
            {{"id":"down","label":"Offline PG","url":"postgresql://viewer:super-secret-pw@127.0.0.1:1/nowhere"}}
        ]}}"#,
        sqlite_url(&root.join("alpha.db")),
        sqlite_url(&root.join("beta.db"))
    );
    fs::write(&profiles, json).unwrap();
    Fixture { root, profiles }
}

fn state(f: &Fixture) -> (Router, AppState) {
    let config = Config::from_values(&[
        ("CUADB_PROFILES_FILE", f.profiles.to_str().unwrap()),
        ("CUADB_SQLITE_ROOT", f.root.to_str().unwrap()),
        ("CUADB_CONNECTION_TIMEOUT_MS", "500"),
        ("CUADB_QUERY_TIMEOUT_MS", "500"),
        ("CUADB_MAX_POOL_CONNECTIONS", "6"),
    ])
    .unwrap();
    let state = AppState::from_config(&config).unwrap();
    (router_with_state(&config.limits, state.clone()), state)
}

async fn get(router: &Router, path: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 22).await.unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn profiles_resolve_by_id_and_default_to_the_configured_one() {
    let f = fixture("resolve").await;
    let (router, state) = state(&f);
    let (_, default_tree) = get(&router, "/catalog/relations?schema=main").await;
    assert!(default_tree.contains("alpha_items") && !default_tree.contains("beta_items"));
    let (_, alpha) = get(&router, "/catalog/relations?schema=main&profile=alpha").await;
    assert!(alpha.contains("alpha_items"));
    let (_, beta) = get(&router, "/catalog/relations?schema=main&profile=beta").await;
    assert!(beta.contains("beta_items") && !beta.contains("alpha_items"));
    let (_, rows) = get(
        &router,
        "/catalog/data?schema=main&relation=beta_items&profile=beta",
    )
    .await;
    assert!(rows.contains("beta-1") && !rows.contains("alpha-1"));
    // The wrong profile cannot see another profile's table.
    let (status, _) = get(
        &router,
        "/catalog/data?schema=main&relation=beta_items&profile=alpha",
    )
    .await;
    assert!(status.is_client_error());
    assert_eq!(state.profiles().len(), 3);
    state.close().await;
}

#[tokio::test]
async fn unknown_removed_and_malformed_profile_ids_are_rejected_without_details() {
    let f = fixture("unknown").await;
    let (router, state) = state(&f);
    for bad in [
        "nope",
        "removed-profile",
        "ALPHA",
        "a%20b",
        "..%2F..%2Fetc",
        "alpha%00",
        &"a".repeat(33),
        "postgres%3A%2F%2Fevil%2Fdb",
    ] {
        for route in [
            "/catalog/schemas?",
            "/catalog/relations?schema=main&",
            "/catalog/data?schema=main&relation=alpha_items&",
            "/catalog/structure?schema=main&relation=alpha_items&",
            "/catalog/export?schema=main&relation=alpha_items&format=csv&",
            "/catalog/overview?",
            "/connection/status?",
            "/connection/profiles?",
        ] {
            let (status, body) = get(&router, &format!("{route}profile={bad}")).await;
            assert!(
                status == StatusCode::NOT_FOUND || status == StatusCode::BAD_REQUEST,
                "{route}{bad} -> {status}"
            );
            assert!(
                !body.contains("sqlite")
                    && !body.contains("super-secret")
                    && !body.contains("127.0.0.1")
            );
        }
    }
    let (status, body) = get(&router, "/catalog/schemas?profile=nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("unknown_profile"));
    // Duplicate profile parameters and connection-like parameters are not accepted.
    for path in [
        "/catalog/relations?schema=main&profile=alpha&profile=beta",
        "/catalog/relations?schema=main&url=sqlite%3A%2F%2F%2Fetc%2Fpasswd",
        "/catalog/data?schema=main&relation=alpha_items&host=evil",
    ] {
        let (status, _) = get(&router, path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
    state.close().await;
}

#[tokio::test]
async fn navigation_links_keep_the_selected_profile() {
    let f = fixture("links").await;
    let (router, state) = state(&f);
    let (_, tree) = get(&router, "/catalog/relations?schema=main&profile=beta").await;
    assert!(
        tree.contains("profile=beta"),
        "relation links must carry the profile"
    );
    let (_, schemas) = get(&router, "/catalog/schemas?profile=beta").await;
    assert!(schemas.contains("profile=beta"));
    let (_, data) = get(
        &router,
        "/catalog/data?schema=main&relation=beta_items&profile=beta",
    )
    .await;
    let (_, structure) = get(
        &router,
        "/catalog/structure?schema=main&relation=beta_items&profile=beta",
    )
    .await;
    for html in [&data, &structure] {
        assert!(html.contains("profile=beta"));
        assert!(!html.contains("profile=alpha"));
    }
    assert!(
        data.contains(r#"name="profile" value="beta""#),
        "search form keeps the profile"
    );
    let (_, plain) = get(&router, "/catalog/data?schema=main&relation=alpha_items").await;
    assert!(
        !plain.contains("profile="),
        "default profile keeps URLs unchanged"
    );
    state.close().await;
}

#[tokio::test]
async fn profile_list_exposes_labels_engine_and_state_but_never_urls_or_paths() {
    let f = fixture("list").await;
    let (router, state) = state(&f);
    let (status, html) = get(&router, "/connection/profiles").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Alpha (SQLite)") && html.contains("Offline PG"));
    assert!(
        html.contains("&lt;b&gt;") || html.contains("&#60;b&#62;"),
        "labels are escaped"
    );
    assert!(!html.contains("<b>(SQLite)"));
    assert!(html.contains(r#"data-profile="alpha""#) && html.contains(r#"aria-selected="true""#));
    for secret in [
        "super-secret-pw",
        "viewer",
        "127.0.0.1",
        "profiles",
        env!("CARGO_TARGET_TMPDIR"),
    ] {
        assert!(!html.contains(secret), "{secret} leaked");
    }
    // The database name is shown on purpose (it is not a credential); host and user never are.
    assert!(html.contains(r#"data-db="nowhere""#));
    assert!(
        html.contains(r#"data-db="alpha.db""#),
        "a SQLite file is shown by name only, never its directory"
    );
    // Connected vs offline states are distinguishable.
    assert!(html.matches("Connected").count() >= 2);
    assert!(html.contains("Unavailable"));
    let (_, chosen) = get(&router, "/connection/profiles?profile=beta").await;
    let selected_beta = chosen.split("data-profile=\"beta\"").nth(1).unwrap();
    assert!(
        selected_beta
            .trim_start()
            .starts_with("aria-selected=\"true\"")
    );
    let (_, status_beta) = get(&router, "/connection/status?profile=beta").await;
    assert!(status_beta.contains("Connected"));
    let (_, status_down) = get(&router, "/connection/status?profile=down").await;
    assert!(!status_down.contains("Connected") && !status_down.contains("super-secret-pw"));
    state.close().await;
}

#[tokio::test]
async fn an_offline_profile_fails_alone_and_rapid_switching_never_mixes_rows() {
    let f = fixture("rapid").await;
    let (router, state) = state(&f);
    let (status, body) = get(&router, "/catalog/schemas?profile=down").await;
    assert!(status.is_success() || status.is_server_error(), "{status}");
    assert!(!body.contains("super-secret-pw") && !body.contains("127.0.0.1"));
    let (status, _) = get(
        &router,
        "/catalog/data?schema=public&relation=t&profile=down",
    )
    .await;
    assert!(status.is_server_error() || status.is_client_error());
    let mut tasks = vec![];
    for i in 0..40 {
        let router = router.clone();
        tasks.push(tokio::spawn(async move {
            let (id, table, expect, other) = match i % 3 {
                0 => ("alpha", "alpha_items", "alpha-1", "beta-1"),
                1 => ("beta", "beta_items", "beta-1", "alpha-1"),
                _ => ("down", "x", "", ""),
            };
            let (status, body) = get(
                &router,
                &format!("/catalog/data?schema=main&relation={table}&profile={id}"),
            )
            .await;
            (id, status, body, expect, other)
        }));
    }
    for task in tasks {
        let (id, status, body, expect, other) = task.await.unwrap();
        if id == "down" {
            assert!(!status.is_success(), "offline profile must not return rows");
        } else {
            assert_eq!(status, StatusCode::OK, "{id}");
            assert!(
                body.contains(expect) && !body.contains(other),
                "{id} leaked another profile's rows"
            );
        }
    }
    state.close().await;
}

#[tokio::test]
async fn single_url_configuration_still_works_as_the_default_profile() {
    let f = fixture("single").await;
    let url = sqlite_url(&f.root.join("alpha.db"));
    let config = Config::from_values(&[
        ("CUADB_DATABASE_URL", url.as_str()),
        ("CUADB_SQLITE_ROOT", f.root.to_str().unwrap()),
    ])
    .unwrap();
    let state = AppState::from_config(&config).unwrap();
    let router = router_with_state(&config.limits, state.clone());
    assert_eq!(state.profiles().len(), 1);
    assert_eq!(state.default_profile_id(), Some("default".to_string()));
    let (_, plain) = get(&router, "/catalog/relations?schema=main").await;
    let (_, named) = get(&router, "/catalog/relations?schema=main&profile=default").await;
    assert!(plain.contains("alpha_items") && named.contains("alpha_items"));
    let (status, _) = get(&router, "/catalog/relations?schema=main&profile=alpha").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    state.close().await;
}

#[tokio::test]
async fn startup_rejects_conflicting_missing_and_invalid_profile_configuration() {
    let f = fixture("startup").await;
    let file = f.profiles.to_str().unwrap();
    let both = Config::from_values(&[
        ("CUADB_PROFILES_FILE", file),
        ("CUADB_DATABASE_URL", "postgresql://h/db"),
    ]);
    assert!(both.is_err(), "file and single URL are mutually exclusive");
    assert!(Config::from_values(&[("CUADB_PROFILES_FILE", "")]).is_err());
    let missing =
        Config::from_values(&[("CUADB_PROFILES_FILE", "/nonexistent/profiles.json")]).unwrap();
    let error = AppState::from_config(&missing).err().unwrap().to_string();
    assert!(!error.contains("/nonexistent"), "paths are not echoed");
    // An invalid URL inside a profile fails startup without echoing it.
    fs::write(
        &f.profiles,
        r#"{"profiles":[{"id":"x","label":"X","url":"postgresql://u:topsecret@h/db?options=-c+bad"}]}"#,
    )
    .unwrap();
    let bad = Config::from_values(&[("CUADB_PROFILES_FILE", file)]).unwrap();
    let error = AppState::from_config(&bad).err().unwrap();
    assert!(
        !error.to_string().contains("topsecret") && !format!("{error:?}").contains("topsecret")
    );
    // A removed profile (file edited and restarted) is simply unknown afterwards.
    fs::write(
        &f.profiles,
        format!(
            r#"{{"profiles":[{{"id":"alpha","label":"A","url":"{}"}}]}}"#,
            sqlite_url(&f.root.join("alpha.db"))
        ),
    )
    .unwrap();
    let config = Config::from_values(&[
        ("CUADB_PROFILES_FILE", file),
        ("CUADB_SQLITE_ROOT", f.root.to_str().unwrap()),
    ])
    .unwrap();
    let state = AppState::from_config(&config).unwrap();
    let router = router_with_state(&config.limits, state.clone());
    let (status, _) = get(&router, "/catalog/relations?schema=main&profile=beta").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    state.close().await;
}

/// Cursors are issued per adapter instance, so a cursor from one profile is useless
/// (and rejected) on another, even when both profiles point at the same database.
#[tokio::test]
async fn pagination_cursors_are_scoped_to_the_profile_that_issued_them() {
    let Ok(url) = std::env::var("CUADB_TEST_ROWS_URL") else {
        return; // not live evidence when unset
    };
    let f = fixture("cursor").await;
    fs::write(
        &f.profiles,
        format!(
            r#"{{"profiles":[{{"id":"one","label":"One","url":"{url}"}},{{"id":"two","label":"Two","url":"{url}"}}]}}"#
        ),
    )
    .unwrap();
    let (router, state) = state(&f);
    let base = "/catalog/data?schema=cuadb_fixture&relation=paging&size=50";
    let (status, first) = get(&router, &format!("{base}&profile=one")).await;
    assert_eq!(status, StatusCode::OK);
    let start = first.find("cursor=").expect("next cursor link");
    let cursor: String = first[start + 7..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();
    let (ok, second) = get(&router, &format!("{base}&profile=one&cursor={cursor}")).await;
    assert_eq!(ok, StatusCode::OK);
    assert!(second.contains("51–100"));
    let (status, body) = get(&router, &format!("{base}&profile=two&cursor={cursor}")).await;
    assert!(
        status.is_client_error(),
        "cursor from profile one must not work on two: {status}"
    );
    assert!(!body.contains("51–100"));
    state.close().await;
}
