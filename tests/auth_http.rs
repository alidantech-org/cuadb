//! Authentication, sessions, CSRF, RBAC and audit through the real router.

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use cuadb::{
    app::router_with_state,
    auth::{
        AuthService, Principal, Role, bootstrap::bootstrap_admin, service::SessionPolicy,
        store::AuthStore,
    },
    config::Config,
    db::{
        adapter::DatabaseAdapter,
        types::{
            Availability, Capability, CatalogList, Cell, Column, DbError, Engine, Namespace,
            PageRequest, Relation, RelationId, RelationKind, RelationStats, RowOptions, RowPage,
        },
    },
    state::AppState,
};
use tower::ServiceExt;

const PW: &str = "a long enough passphrase";

struct Fake;

#[async_trait::async_trait]
impl DatabaseAdapter for Fake {
    fn engine(&self) -> Engine {
        Engine::Sqlite
    }
    fn supports(&self, _: Capability) -> bool {
        false
    }
    async fn namespaces(&self) -> Result<Vec<Namespace>, DbError> {
        Ok(["main", "secret"]
            .into_iter()
            .map(|n| Namespace { name: n.into() })
            .collect())
    }
    async fn relations(&self, schema: &str) -> Result<Vec<Relation>, DbError> {
        Ok(["t", "u"]
            .into_iter()
            .map(|n| Relation {
                id: RelationId {
                    namespace: schema.into(),
                    name: n.into(),
                },
                kind: RelationKind::Table,
            })
            .collect())
    }
    async fn columns(&self, _: &RelationId) -> Result<Vec<Column>, DbError> {
        Ok(vec![])
    }
    async fn statistics(&self, _: &RelationId) -> Result<RelationStats, DbError> {
        Err(DbError::new(cuadb::db::types::DbErrorKind::Unsupported))
    }
    async fn page(&self, _: &RelationId, _: RowOptions) -> Result<RowPage, DbError> {
        let col = Column {
            name: "id".into(),
            native_type: "integer".into(),
            nullable: false,
            primary_key: true,
            default_expression: None,
        };
        Ok(RowPage {
            available_columns: vec![col.clone()],
            columns: vec![col],
            rows: vec![vec![Cell::Integer(1)]],
            has_more: false,
            approximate_total: Availability::Unavailable,
            previews: vec![vec![cuadb::db::types::RowPreview {
                text: "1".into(),
                truncated: false,
                type_name: "integer".into(),
                detail: None,
            }]],
            has_previous: false,
            next_cursor: None,
            previous_cursor: None,
            stable_order: true,
            keyset_enabled: false,
            page: 0,
            offset: 0,
            truncated_columns: false,
        })
    }
    async fn rows(&self, _: &RelationId, _: PageRequest) -> Result<RowPage, DbError> {
        Err(DbError::new(cuadb::db::types::DbErrorKind::Unsupported))
    }
    async fn constraints(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<cuadb::db::types::Constraint>, DbError> {
        use cuadb::db::types::{Constraint, ConstraintKind};
        let items = if id.name == "t" {
            vec![Constraint {
                name: "t_u_fk".into(),
                kind: ConstraintKind::ForeignKey,
                columns: vec!["id".into()],
                references: Some(RelationId {
                    namespace: id.namespace.clone(),
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
    async fn list_schemas(&self) -> Result<CatalogList<Namespace>, DbError> {
        Ok(CatalogList {
            items: self.namespaces().await?,
            truncated: false,
        })
    }
}

struct World {
    router: Router,
    svc: AuthService,
    admin: Principal,
}

async fn world(enforce: bool) -> World {
    let store = AuthStore::open_memory().await.unwrap();
    let svc = AuthService::new(
        store,
        SessionPolicy {
            idle_secs: 1800,
            max_secs: 43_200,
            cookie_secure: false,
        },
        false,
    )
    .await
    .unwrap();
    if enforce {
        bootstrap_admin(&svc.store, "admin", PW).await.unwrap();
        svc.refresh_enforced().await.unwrap();
    }
    let state = AppState::with_adapters(vec![
        ("alpha", "Alpha", Arc::new(Fake) as Arc<dyn DatabaseAdapter>),
        ("beta", "Beta", Arc::new(Fake) as Arc<dyn DatabaseAdapter>),
    ])
    .with_auth(svc.clone());
    let limits = Config::from_values(&[]).unwrap().limits;
    let router = router_with_state(&limits, state);
    let admin = if enforce {
        svc.login("admin", PW).await.unwrap().principal
    } else {
        Principal::open()
    };
    World { router, svc, admin }
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: String,
}

async fn send(router: &Router, mut request: Request<Body>) -> Reply {
    request
        .headers_mut()
        .entry(header::HOST)
        .or_insert("localhost:4848".parse().unwrap());
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    Reply {
        status,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

async fn get(router: &Router, path: &str, cookie: Option<&str>) -> Reply {
    let mut b = Request::get(path);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    send(router, b.body(Body::empty()).unwrap()).await
}

async fn post(
    router: &Router,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: &str,
) -> Reply {
    let mut b = Request::post(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://localhost:4848");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    let body = match csrf {
        Some(t) => format!("{body}&csrf={t}"),
        None => body.to_string(),
    };
    send(router, b.body(Body::from(body)).unwrap()).await
}

fn cookie_of(reply: &Reply) -> String {
    let raw = reply
        .headers
        .get(header::SET_COOKIE)
        .expect("Set-Cookie")
        .to_str()
        .unwrap();
    raw.split(';').next().unwrap().to_string()
}

#[tokio::test]
async fn without_an_administrator_the_local_shell_stays_open_but_read_only() {
    let w = world(false).await;
    assert_eq!(
        get(&w.router, "/catalog/schemas", None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        get(&w.router, "/catalog/data?schema=main&relation=t", None)
            .await
            .status,
        StatusCode::OK
    );
    for path in ["/admin/users", "/admin/audit", "/admin/grants?user=1"] {
        assert_eq!(
            get(&w.router, path, None).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    let r = post(
        &w.router,
        "/admin/users",
        None,
        None,
        "login=eve&password=a+long+enough+passphrase&role=admin",
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::FORBIDDEN,
        "no registration endpoint while auth is unconfigured"
    );
    assert_eq!(w.svc.store.user_count().await.unwrap(), 0);
}

#[tokio::test]
async fn once_enforced_every_data_route_requires_a_session() {
    let w = world(true).await;
    for path in [
        "/catalog/schemas",
        "/catalog/relations?schema=main",
        "/catalog/data?schema=main&relation=t",
        "/catalog/structure?schema=main&relation=t",
        "/catalog/export?schema=main&relation=t&format=csv",
        "/catalog/overview",
        "/connection/profiles",
        "/connection/status",
        "/admin/users",
        "/",
    ] {
        let r = get(&w.router, path, None).await;
        assert_eq!(
            r.status,
            StatusCode::SEE_OTHER,
            "{path} navigations redirect to login"
        );
        assert_eq!(r.headers.get(header::LOCATION).unwrap(), "/login");
        assert!(!r.body.contains("Alpha"));
        // HTMX requests get 401 plus a redirect hint instead of a swapped login page.
        let hx = send(
            &w.router,
            Request::get(path)
                .header("hx-request", "true")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(hx.status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(hx.headers.get("hx-redirect").unwrap(), "/login");
    }
    // Public routes stay reachable.
    assert_eq!(get(&w.router, "/health", None).await.status, StatusCode::OK);
    assert_eq!(get(&w.router, "/login", None).await.status, StatusCode::OK);
    // Garbage, forged and wrong-shaped cookies never authenticate.
    for cookie in [
        "cuadb_session=abc",
        &format!("cuadb_session={}", "a".repeat(64)),
        "cuadb_session=",
        "other=1",
    ] {
        assert_eq!(
            get(&w.router, "/catalog/schemas", Some(cookie))
                .await
                .status,
            StatusCode::SEE_OTHER
        );
    }
}

#[tokio::test]
async fn login_sets_a_safe_cookie_and_a_new_session_every_time() {
    let w = world(true).await;
    let bad = post(
        &w.router,
        "/login",
        None,
        None,
        "login=admin&password=wrong+wrong+wrong",
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert!(bad.headers.get(header::SET_COOKIE).is_none());
    assert!(
        !bad.body.contains("admin"),
        "no hint about which part was wrong"
    );
    let ok = post(
        &w.router,
        "/login",
        None,
        None,
        "login=admin&password=a+long+enough+passphrase",
    )
    .await;
    assert_eq!(ok.status, StatusCode::SEE_OTHER);
    let raw = ok
        .headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    for flag in ["HttpOnly", "SameSite=Strict", "Path=/", "Max-Age=43200"] {
        assert!(raw.contains(flag), "{flag} in {raw}");
    }
    assert_eq!(ok.headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
    let cookie = cookie_of(&ok);
    assert_eq!(
        get(&w.router, "/catalog/schemas", Some(&cookie))
            .await
            .status,
        StatusCode::OK
    );
    // Fixation: an attacker-chosen cookie is ignored; login issues a fresh random token.
    let planted = format!("cuadb_session={}", "ab".repeat(32));
    let again = post(
        &w.router,
        "/login",
        Some(&planted),
        None,
        "login=admin&password=a+long+enough+passphrase",
    )
    .await;
    let fresh = cookie_of(&again);
    assert_ne!(fresh, planted);
    assert_ne!(fresh, cookie);
    assert_eq!(
        get(&w.router, "/catalog/schemas", Some(&planted))
            .await
            .status,
        StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn secure_flag_follows_the_policy() {
    let store = AuthStore::open_memory().await.unwrap();
    let svc = AuthService::new(
        store,
        SessionPolicy {
            idle_secs: 60,
            max_secs: 600,
            cookie_secure: true,
        },
        false,
    )
    .await
    .unwrap();
    bootstrap_admin(&svc.store, "admin", PW).await.unwrap();
    svc.refresh_enforced().await.unwrap();
    let state = AppState::with_adapter(Arc::new(Fake)).with_auth(svc);
    let router = router_with_state(&Config::from_values(&[]).unwrap().limits, state);
    let ok = post(
        &router,
        "/login",
        None,
        None,
        "login=admin&password=a+long+enough+passphrase",
    )
    .await;
    assert!(
        ok.headers
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("; Secure")
    );
}

#[tokio::test]
async fn csrf_and_origin_are_enforced_on_every_state_changing_request() {
    let w = world(true).await;
    let login = w.svc.login("admin", PW).await.unwrap();
    let cookie = format!("cuadb_session={}", login.token);
    let csrf = login.csrf.clone();
    let form = "login=zed&password=a+totally+different+secret&role=viewer";
    assert_eq!(
        post(&w.router, "/admin/users", Some(&cookie), None, form)
            .await
            .status,
        StatusCode::FORBIDDEN,
        "missing token"
    );
    assert_eq!(
        post(
            &w.router,
            "/admin/users",
            Some(&cookie),
            Some("0".repeat(64).as_str()),
            form
        )
        .await
        .status,
        StatusCode::FORBIDDEN,
        "wrong token"
    );
    assert_eq!(
        post(
            &w.router,
            "/admin/users",
            Some(&cookie),
            Some(&csrf[..63]),
            form
        )
        .await
        .status,
        StatusCode::FORBIDDEN,
        "short token"
    );
    // A cross-site Origin fails even with a valid token.
    let cross = send(
        &w.router,
        Request::post("/admin/users")
            .header(header::COOKIE, &cookie)
            .header(header::ORIGIN, "https://evil.example")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("{form}&csrf={csrf}")))
            .unwrap(),
    )
    .await;
    assert_eq!(cross.status, StatusCode::FORBIDDEN);
    let fetch_site = send(
        &w.router,
        Request::post("/admin/users")
            .header(header::COOKIE, &cookie)
            .header("sec-fetch-site", "cross-site")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!("{form}&csrf={csrf}")))
            .unwrap(),
    )
    .await;
    assert_eq!(fetch_site.status, StatusCode::FORBIDDEN);
    assert!(
        w.svc.store.user_by_login("zed").await.unwrap().is_none(),
        "nothing was created"
    );
    // The right token works, also via header.
    assert_eq!(
        post(&w.router, "/admin/users", Some(&cookie), Some(&csrf), form)
            .await
            .status,
        StatusCode::OK
    );
    let via_header = send(
        &w.router,
        Request::post("/admin/users/status")
            .header(header::COOKIE, &cookie)
            .header(header::ORIGIN, "http://localhost:4848")
            .header("x-csrf-token", &csrf)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("user_id=999&active=false"))
            .unwrap(),
    )
    .await;
    assert_eq!(
        via_header.status,
        StatusCode::BAD_REQUEST,
        "authorised, then rejected as an unknown user"
    );
    // GETs never need a token, and unauthenticated POSTs are refused before parsing.
    assert_eq!(
        post(&w.router, "/admin/users", None, None, form)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn logout_revokes_the_session_and_expired_sessions_are_rejected() {
    let w = world(true).await;
    let login = w.svc.login("admin", PW).await.unwrap();
    let cookie = format!("cuadb_session={}", login.token);
    let out = post(&w.router, "/logout", Some(&cookie), Some(&login.csrf), "").await;
    assert_eq!(out.status, StatusCode::SEE_OTHER);
    assert!(
        out.headers
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        get(&w.router, "/catalog/schemas", Some(&cookie))
            .await
            .status,
        StatusCode::SEE_OTHER,
        "revoked"
    );
    let second = w.svc.login("admin", PW).await.unwrap();
    let cookie2 = format!("cuadb_session={}", second.token);
    sqlx::query("UPDATE sessions SET expires_at = 1")
        .execute(&w.svc.store.pool)
        .await
        .unwrap();
    assert_eq!(
        get(&w.router, "/catalog/schemas", Some(&cookie2))
            .await
            .status,
        StatusCode::SEE_OTHER,
        "expired"
    );
}

async fn user_cookie(
    w: &World,
    login: &str,
    role: Role,
    grants: &[(&str, &str, &str, u8)],
) -> (String, String) {
    let id = w
        .svc
        .create_user(&w.admin, login, "a long enough passphrase!", role)
        .await
        .unwrap();
    for (profile, schema, relation, actions) in grants {
        w.svc
            .add_grant(&w.admin, id, profile, schema, relation, *actions)
            .await
            .unwrap();
    }
    let session = w
        .svc
        .login(login, "a long enough passphrase!")
        .await
        .unwrap();
    (format!("cuadb_session={}", session.token), session.csrf)
}

#[tokio::test]
async fn users_without_grants_see_nothing_and_grants_scope_to_profile_schema_and_relation() {
    let w = world(true).await;
    let (none, _) = user_cookie(&w, "nobody", Role::Viewer, &[]).await;
    for path in [
        "/catalog/schemas?profile=alpha",
        "/catalog/relations?schema=main&profile=alpha",
        "/catalog/data?schema=main&relation=t&profile=alpha",
        "/catalog/export?schema=main&relation=t&format=csv&profile=alpha",
        "/catalog/overview?profile=alpha",
        "/connection/status?profile=alpha",
    ] {
        assert_eq!(
            get(&w.router, path, Some(&none)).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    let listed = get(&w.router, "/connection/profiles", Some(&none)).await;
    assert!(
        !listed.body.contains("Alpha") && !listed.body.contains("Beta"),
        "profiles without a grant are not even listed"
    );

    // viewer: view+export on alpha / main / t only.
    let (reader, _) = user_cookie(&w, "reader", Role::Viewer, &[("alpha", "main", "t", 3)]).await;
    let ok = get(
        &w.router,
        "/catalog/data?schema=main&relation=t&profile=alpha",
        Some(&reader),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    assert_eq!(
        get(
            &w.router,
            "/catalog/export?schema=main&relation=t&format=csv&profile=alpha",
            Some(&reader)
        )
        .await
        .status,
        StatusCode::OK
    );
    // Same schema, other relation; other schema; other profile: all denied.
    for path in [
        "/catalog/data?schema=main&relation=u&profile=alpha",
        "/catalog/data?schema=secret&relation=t&profile=alpha",
        "/catalog/data?schema=main&relation=t&profile=beta",
        "/catalog/structure?schema=main&relation=u&profile=alpha",
        "/catalog/overview?profile=alpha",
        "/catalog/advanced?schema=main&profile=alpha",
        "/catalog/relations?schema=secret&profile=alpha",
    ] {
        let r = get(&w.router, path, Some(&reader)).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{path}");
        assert!(!r.body.contains("secret") || r.body.contains("access_denied"));
    }
    // Lists are filtered, not just guarded.
    let schemas = get(&w.router, "/catalog/schemas?profile=alpha", Some(&reader))
        .await
        .body;
    assert!(schemas.contains("main") && !schemas.contains("secret"));
    let relations = get(
        &w.router,
        "/catalog/relations?schema=main&profile=alpha",
        Some(&reader),
    )
    .await
    .body;
    assert!(relations.contains(">t<") || relations.contains("data-relation-name=\"t\""));
    assert!(!relations.contains("data-relation-name=\"u\""));
    let profiles = get(&w.router, "/connection/profiles", Some(&reader))
        .await
        .body;
    assert!(profiles.contains("Alpha") && !profiles.contains("Beta"));
}

#[tokio::test]
async fn roles_cap_what_grants_can_do_and_admin_pages_are_admin_only() {
    let w = world(true).await;
    let (viewer, vcsrf) = user_cookie(&w, "vera", Role::Viewer, &[("alpha", "", "", 15)]).await;
    for path in ["/admin/users", "/admin/audit", "/admin/grants?user=1"] {
        assert_eq!(
            get(&w.router, path, Some(&viewer)).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    let escalate = post(
        &w.router,
        "/admin/grants",
        Some(&viewer),
        Some(&vcsrf),
        "user_id=2&profile=alpha&actions=view",
    )
    .await;
    assert_eq!(escalate.status, StatusCode::FORBIDDEN);
    let promote = post(
        &w.router,
        "/admin/users",
        Some(&viewer),
        Some(&vcsrf),
        "login=evil&password=a+long+enough+passphrase&role=admin",
    )
    .await;
    assert_eq!(promote.status, StatusCode::FORBIDDEN);
    assert!(w.svc.store.user_by_login("evil").await.unwrap().is_none());
    // A whole-profile grant of all actions is capped by the viewer role: view/export only.
    let me = w.svc.store.user_by_login("vera").await.unwrap().unwrap();
    let principal = w.svc.grants(me.id).await.unwrap();
    assert_eq!(principal[0].actions, 15, "stored as granted");
    assert_eq!(
        get(
            &w.router,
            "/catalog/data?schema=main&relation=t&profile=alpha",
            Some(&viewer)
        )
        .await
        .status,
        StatusCode::OK
    );
    let overview = get(&w.router, "/catalog/overview?profile=alpha", Some(&viewer)).await;
    assert_ne!(
        overview.status,
        StatusCode::FORBIDDEN,
        "a profile-wide grant is authorised (the fake adapter then reports it unsupported)"
    );
}

#[tokio::test]
async fn admin_can_manage_users_and_grants_over_http_and_the_audit_trail_records_it() {
    let w = world(true).await;
    let login = w.svc.login("admin", PW).await.unwrap();
    let cookie = format!("cuadb_session={}", login.token);
    let csrf = login.csrf;
    assert_eq!(
        post(
            &w.router,
            "/admin/users",
            Some(&cookie),
            Some(&csrf),
            "login=erin&password=another+unrelated+secret+99&role=editor"
        )
        .await
        .status,
        StatusCode::OK
    );
    let erin = w.svc.store.user_by_login("erin").await.unwrap().unwrap();
    assert!(!erin.password_hash.contains("another"));
    let listing = get(&w.router, "/admin/users", Some(&cookie)).await.body;
    assert!(listing.contains("erin") && listing.contains("editor") && !listing.contains("argon2"));
    let grant = post(
        &w.router,
        "/admin/grants",
        Some(&cookie),
        Some(&csrf),
        &format!(
            "user_id={}&profile=alpha&schema=main&actions=view,export",
            erin.id
        ),
    )
    .await;
    assert_eq!(grant.status, StatusCode::OK);
    assert_eq!(
        post(
            &w.router,
            "/admin/grants",
            Some(&cookie),
            Some(&csrf),
            &format!("user_id={}&profile=nope&actions=view", erin.id)
        )
        .await
        .status,
        StatusCode::NOT_FOUND,
        "unknown profile"
    );
    for bad in [
        "actions=drop",
        "actions=",
        "schema=&relation=t&actions=view",
    ] {
        let r = post(
            &w.router,
            "/admin/grants",
            Some(&cookie),
            Some(&csrf),
            &format!("user_id={}&profile=alpha&{bad}", erin.id),
        )
        .await;
        assert!(r.status.is_client_error(), "{bad}");
    }
    let grants_page = get(
        &w.router,
        &format!("/admin/grants?user={}", erin.id),
        Some(&cookie),
    )
    .await
    .body;
    assert!(grants_page.contains("view,export") && grants_page.contains("alpha"));
    // Revoke through the grant id.
    let gid = w.svc.grants(erin.id).await.unwrap()[0].id;
    assert_eq!(
        post(
            &w.router,
            "/admin/grants/remove",
            Some(&cookie),
            Some(&csrf),
            &format!("grant_id={gid}")
        )
        .await
        .status,
        StatusCode::OK
    );
    assert!(w.svc.grants(erin.id).await.unwrap().is_empty());

    let audit = get(&w.router, "/admin/audit?limit=100", Some(&cookie))
        .await
        .body;
    for needle in ["login_ok", "user_created", "grant_added", "grant_removed"] {
        assert!(audit.contains(needle), "{needle} missing from audit");
    }
    for secret in ["another+unrelated", "argon2", &login.token, &csrf] {
        assert!(
            !audit.contains(secret),
            "{secret} leaked into the audit view"
        );
    }
}

#[tokio::test]
async fn denied_access_and_failed_logins_are_audited_and_the_log_is_bounded_and_append_only() {
    let w = world(true).await;
    let (reader, _) = user_cookie(&w, "rita", Role::Viewer, &[("alpha", "main", "t", 1)]).await;
    let _ = get(
        &w.router,
        "/catalog/data?schema=main&relation=u&profile=alpha",
        Some(&reader),
    )
    .await;
    let _ = get(
        &w.router,
        "/catalog/data?schema=main&relation=t&profile=beta",
        Some(&reader),
    )
    .await;
    let _ = post(
        &w.router,
        "/login",
        None,
        None,
        "login=rita&password=not+the+password+x",
    )
    .await;
    let events = w.svc.audit_events(200, None).await.unwrap();
    let denied: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "access_denied")
        .collect();
    assert!(
        denied.len() >= 2
            && denied
                .iter()
                .all(|e| e.actor == "rita" && e.outcome == "denied")
    );
    assert!(
        events
            .iter()
            .any(|e| e.kind == "login_failed" && e.actor == "rita")
    );
    assert!(denied.iter().any(|e| e.resource.contains("beta")));
    // Bounded: limit is clamped and pagination continues with `before`.
    assert!(w.svc.audit_events(10_000, None).await.unwrap().len() <= 200);
    let first = w.svc.audit_events(2, None).await.unwrap();
    let next = w
        .svc
        .audit_events(2, Some(first.last().unwrap().id))
        .await
        .unwrap();
    assert!(next.iter().all(|e| e.id < first.last().unwrap().id));
    // Append-only: no UPDATE, and no DELETE inside the retention window.
    assert!(
        sqlx::query("UPDATE audit_events SET outcome = 'ok'")
            .execute(&w.svc.store.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM audit_events")
            .execute(&w.svc.store.pool)
            .await
            .is_err()
    );
    assert_eq!(
        w.svc.store.prune_audit().await.unwrap(),
        0,
        "nothing is old enough to prune"
    );
}

#[tokio::test]
async fn related_zip_never_follows_or_reveals_relations_the_user_may_not_export() {
    let w = world(true).await;
    let zip = "/catalog/export?schema=main&relation=t&format=zip&scope=related&profile=alpha";
    let (narrow, _) = user_cookie(&w, "nora", Role::Viewer, &[("alpha", "main", "t", 3)]).await;
    let reply = send(
        &w.router,
        Request::get(zip)
            .header(header::COOKIE, &narrow)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.body.contains("main_t.csv"));
    assert!(
        !reply.body.contains("main_u.csv"),
        "linked relation without a grant is not exported"
    );
    assert!(
        !reply.body.contains("t_u_fk") && !reply.body.contains("main.u"),
        "nor mentioned in the manifest"
    );
    let (wide, _) = user_cookie(&w, "wade", Role::Viewer, &[("alpha", "main", "", 3)]).await;
    let both = send(
        &w.router,
        Request::get(zip)
            .header(header::COOKIE, &wide)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(
        both.body.contains("main_u.csv"),
        "a schema-wide grant includes the linked relation"
    );
    // View-only users cannot export at all.
    let (view_only, _) = user_cookie(&w, "vic", Role::Viewer, &[("alpha", "", "", 1)]).await;
    let denied = send(
        &w.router,
        Request::get(zip)
            .header(header::COOKIE, &view_only)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    let events = w.svc.audit_events(200, None).await.unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.kind == "export" && e.actor == "nora" && e.detail == "zip")
    );
}

fn assert_csp_clean(html: &str, what: &str) {
    for needle in [
        " style=",
        "<style",
        "<script>",
        " onclick=",
        " onload=",
        " onchange=",
    ] {
        assert!(!html.contains(needle), "{what} contains `{needle}`");
    }
}

#[tokio::test]
async fn the_shell_carries_the_csrf_token_and_account_controls_for_signed_in_users() {
    let w = world(true).await;
    let login = post(
        &w.router,
        "/login",
        None,
        None,
        &format!("login=admin&password={}", PW.replace(' ', "+")),
    )
    .await;
    let cookie = cookie_of(&login);
    let page = get(&w.router, "/", Some(&cookie)).await;
    assert_eq!(page.status, StatusCode::OK);
    let csrf = w
        .svc
        .login("admin", PW)
        .await
        .unwrap()
        .principal
        .csrf()
        .to_string();
    // Each login mints its own session, so the token of *this* session is in the page.
    assert!(page.body.contains(r#"<meta name="csrf-token" content=""#));
    assert!(
        !page.body.contains(&format!("content=\"{csrf}\"")),
        "another session's token"
    );
    assert!(
        page.body.contains("Sign out"),
        "account block with sign out"
    );
    assert!(
        page.body.contains("Administration"),
        "admins see the administration entry"
    );
    assert!(!page.body.contains("Trusted local session"));
    assert_csp_clean(&page.body, "explorer page");
    assert_eq!(
        page.headers.get(header::REFERRER_POLICY).unwrap(),
        "same-origin",
        "no-referrer would make browsers send `Origin: null` on form posts"
    );
}

#[tokio::test]
async fn local_mode_has_no_sign_out_and_no_administration_entry() {
    let w = world(false).await;
    let page = get(&w.router, "/", None).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("Trusted local session"));
    assert!(!page.body.contains("Sign out"));
    assert!(!page.body.contains("Administration"));
    assert_csp_clean(&page.body, "local explorer page");
}

#[tokio::test]
async fn the_login_page_is_a_complete_document_without_inline_code() {
    let w = world(true).await;
    let page = get(&w.router, "/login", None).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.starts_with("<!doctype html>"));
    assert!(page.body.contains(r#"name="password" type="password""#));
    assert_csp_clean(&page.body, "login page");
}

#[tokio::test]
async fn data_and_catalog_fragments_stay_free_of_inline_style_and_script() {
    let w = world(false).await;
    for path in [
        "/catalog/schemas",
        "/catalog/relations?schema=main",
        "/catalog/data?schema=main&relation=t",
        "/connection/profiles",
    ] {
        let r = get(&w.router, path, None).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert_csp_clean(&r.body, path);
    }
}

#[tokio::test]
async fn the_administration_view_is_admin_only_and_free_of_inline_code() {
    let w = world(true).await;
    let login = post(
        &w.router,
        "/login",
        None,
        None,
        &format!("login=admin&password={}", PW.replace(' ', "+")),
    )
    .await;
    let cookie = cookie_of(&login);
    for (path, needle) in [
        ("/admin", r#"id="admin-body""#),
        ("/admin/users", "Add member"),
        ("/admin/grants?user=0", "Pick a member"),
        ("/admin/audit", "Audit log"),
        ("/admin/security", "Deployment mode"),
    ] {
        let r = get(&w.router, path, Some(&cookie)).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert!(r.body.contains(needle), "{path} lacks {needle}");
        assert_csp_clean(&r.body, path);
    }
    // The signed-in admin cannot disable or reset themself from the list.
    let users = get(&w.router, "/admin/users", Some(&cookie)).await;
    assert!(users.body.contains(">you<") && !users.body.contains("Disable"));
    // Audit rows carry a machine timestamp the browser localizes.
    assert!(
        get(&w.router, "/admin/audit", Some(&cookie))
            .await
            .body
            .contains("<time data-ts=")
    );
    // A successful change tells the page to refresh the list it altered.
    // The token of *this* session, exactly as the page hands it to the browser.
    let shell = get(&w.router, "/", Some(&cookie)).await.body;
    let csrf = shell
        .split(r#"name="csrf-token" content=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap()
        .to_string();
    let created = post(
        &w.router,
        "/admin/users",
        Some(&cookie),
        Some(&csrf),
        "login=zed&password=another+long+passphrase&role=viewer",
    )
    .await;
    assert_eq!(created.status, StatusCode::OK);
    assert_eq!(created.headers.get("hx-trigger").unwrap(), "cuadb:changed");
    // The new member now appears with actions, and the permissions picker lists them.
    let users = get(&w.router, "/admin/users", Some(&cookie)).await;
    assert!(users.body.contains("zed") && users.body.contains("Disable"));
    let picker = get(&w.router, "/admin/grants?user=0", Some(&cookie)).await;
    assert!(picker.body.contains("zed"));
}

#[tokio::test]
async fn non_admins_and_anonymous_visitors_never_get_the_administration_view() {
    let w = world(true).await;
    for path in ["/admin", "/admin/security", "/admin/users"] {
        let anon = get(&w.router, path, None).await;
        assert_eq!(anon.status, StatusCode::SEE_OTHER, "{path} anonymous");
    }
    let viewer = w
        .svc
        .create_user(&w.admin, "vic", "a long enough passphrase!", Role::Viewer)
        .await
        .unwrap();
    let _ = viewer;
    let session = w
        .svc
        .login("vic", "a long enough passphrase!")
        .await
        .unwrap();
    let cookie = format!("cuadb_session={}", session.token);
    for path in ["/admin", "/admin/security", "/admin/users", "/admin/audit"] {
        assert_eq!(
            get(&w.router, path, Some(&cookie)).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
}

async fn admin_session(w: &World) -> (String, String) {
    let login = post(
        &w.router,
        "/login",
        None,
        None,
        &format!("login=admin&password={}", PW.replace(' ', "+")),
    )
    .await;
    let cookie = cookie_of(&login);
    let shell = get(&w.router, "/", Some(&cookie)).await.body;
    let csrf = shell
        .split(r#"name="csrf-token" content=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap()
        .to_string();
    (cookie, csrf)
}

#[tokio::test]
async fn roles_change_and_members_are_deleted_with_last_admin_and_self_protection() {
    let w = world(true).await;
    let (cookie, csrf) = admin_session(&w).await;
    let me = w
        .svc
        .store
        .list_users()
        .await
        .unwrap()
        .into_iter()
        .find(|u| u.login == "admin")
        .unwrap()
        .id;
    let other = w
        .svc
        .create_user(&w.admin, "casey", "a long enough passphrase!", Role::Viewer)
        .await
        .unwrap();
    // Promote, then check the sidebar of that member really changed (role cap lifts).
    let r = post(
        &w.router,
        "/admin/users/role",
        Some(&cookie),
        Some(&csrf),
        &format!("user_id={other}&role=editor"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.headers.get("hx-trigger").unwrap(), "cuadb:changed");
    assert_eq!(
        w.svc.store.user_by_id(other).await.unwrap().unwrap().role,
        "editor"
    );
    // Not yourself, not the last administrator, not an unknown role.
    let r = post(
        &w.router,
        "/admin/users/role",
        Some(&cookie),
        Some(&csrf),
        &format!("user_id={me}&role=viewer"),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.body.contains("your own role"));
    let r = post(
        &w.router,
        "/admin/users/delete",
        Some(&cookie),
        Some(&csrf),
        &format!("user_id={me}"),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.body.contains("own account"));
    let r = post(
        &w.router,
        "/admin/users/role",
        Some(&cookie),
        Some(&csrf),
        &format!("user_id={other}&role=root"),
    )
    .await;
    assert_ne!(r.status, StatusCode::OK);
    // A second admin can be demoted again; the last one cannot be demoted or deleted by another admin.
    w.svc.store.set_user_role(other, "admin").await.unwrap();
    let r = post(
        &w.router,
        "/admin/users/role",
        Some(&cookie),
        Some(&csrf),
        &format!("user_id={other}&role=viewer"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    // Grants and sessions go with the member; the audit history stays.
    w.svc
        .add_grant(&w.admin, other, "alpha", "", "", 1)
        .await
        .unwrap();
    let s = w
        .svc
        .login("casey", "a long enough passphrase!")
        .await
        .unwrap();
    let r = post(
        &w.router,
        "/admin/users/delete",
        Some(&cookie),
        Some(&csrf),
        &format!("user_id={other}"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert!(w.svc.store.user_by_id(other).await.unwrap().is_none());
    assert!(w.svc.store.grants_for_user(other).await.unwrap().is_empty());
    assert_eq!(
        get(
            &w.router,
            "/catalog/schemas",
            Some(&format!("cuadb_session={}", s.token))
        )
        .await
        .status,
        StatusCode::SEE_OTHER,
        "their session ended"
    );
    let audit = get(&w.router, "/admin/audit", Some(&cookie)).await.body;
    assert!(
        audit.contains("user_deleted")
            && audit.contains("user_role_changed")
            && audit.contains("casey")
    );
}

#[tokio::test]
async fn the_audit_log_filters_by_user_action_outcome_and_date() {
    let w = world(true).await;
    let (cookie, csrf) = admin_session(&w).await;
    let _ = post(
        &w.router,
        "/login",
        None,
        None,
        "login=admin&password=wrong+wrong+wrong+x",
    )
    .await;
    let _ = post(
        &w.router,
        "/admin/users",
        Some(&cookie),
        Some(&csrf),
        "login=zed&password=another+long+passphrase&role=viewer",
    )
    .await;
    let all = get(&w.router, "/admin/audit", Some(&cookie)).await.body;
    assert!(all.contains("<td>login_failed</td>") && all.contains("<td>user_created</td>"));
    let by_kind = get(&w.router, "/admin/audit?kind=user_created", Some(&cookie))
        .await
        .body;
    assert!(
        by_kind.contains("<td>user_created</td>") && !by_kind.contains("<td>login_failed</td>")
    );
    assert!(
        by_kind.contains(r#"<option value="user_created" selected>"#),
        "filter state is kept"
    );
    let by_outcome = get(&w.router, "/admin/audit?outcome=failed", Some(&cookie))
        .await
        .body;
    assert!(
        !by_outcome.contains("<td>user_created</td>")
            && by_outcome.contains("<td>login_failed</td>")
    );
    let by_actor = get(&w.router, "/admin/audit?actor=nobody", Some(&cookie))
        .await
        .body;
    assert!(by_actor.contains("No events match."));
    let past = get(&w.router, "/admin/audit?until=2001-01-01", Some(&cookie))
        .await
        .body;
    assert!(past.contains("No events match."));
    let future = get(
        &w.router,
        "/admin/audit?since=2001-01-01&until=2999-12-31",
        Some(&cookie),
    )
    .await
    .body;
    assert!(future.contains("<td>user_created</td>"));
    // Unknown kinds and malformed dates are ignored rather than echoed or executed.
    let junk = get(
        &w.router,
        "/admin/audit?kind=%27%3Bdrop&since=nope",
        Some(&cookie),
    )
    .await
    .body;
    assert!(junk.contains("<td>user_created</td>") && !junk.contains("drop"));
}
