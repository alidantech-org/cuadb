//! Authentication middleware, the `Auth` extractor and the sign-in / administration
//! handlers. Every handler that touches data takes `Auth` and calls `authorize`.

use askama::Template;
use axum::{
    Form,
    body::{Body, to_bytes},
    extract::{FromRequestParts, Query, Request, State},
    http::{HeaderValue, Method, StatusCode, header, request::Parts},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};

use crate::{
    auth::{
        Action, AuthError, Principal, Resource, Role,
        audit::{self, AuditEvent, StoredEvent, kind},
        rbac::{self, Grant},
        session,
    },
    error::{AppError, ErrorKind},
    state::AppState,
};

const MAX_FORM_BYTES: usize = 65_536;

fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn is_anonymous_app_path(path: &str) -> bool {
    path == "/health"
        || path.starts_with("/static/")
        || path.starts_with("/icons/")
        || path == "/manifest.webmanifest"
        || path == "/sw.js"
        || path == "/login"
        || path == "/"
        || path.starts_with("/catalog/")
        || path.starts_with("/table/")
        || path.starts_with("/overview")
        || path.starts_with("/object/")
        || path.starts_with("/connection/")
        || path.starts_with("/admin/connections")
        || path == "/workspace/admin/connections"
        || path == "/sql"
        || path.starts_with("/sql/")
        || path.starts_with("/data/")
        || path.starts_with("/tx/")
}

/// Browsers send `Origin` on unsafe requests; it must be this server's own authority.
fn same_origin(request: &Request) -> bool {
    let headers = request.headers();
    if let Some(origin) = headers.get(header::ORIGIN) {
        let origin = origin.to_str().unwrap_or_default();
        let authority = origin
            .split_once("://")
            .map_or("", |(_, rest)| rest.trim_end_matches('/'));
        let host = headers
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or_default();
        return !authority.is_empty() && authority == host;
    }
    // No Origin: refuse only when the browser says the request is cross-site.
    !matches!(
        headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()),
        Some("cross-site")
    )
}

fn deny(request_is_html_navigation: bool, htmx: bool, kind: ErrorKind) -> Response {
    if kind == ErrorKind::Unauthorized {
        if htmx {
            let mut response = AppError::new(kind).into_response();
            response
                .headers_mut()
                .insert("hx-redirect", HeaderValue::from_static("/login"));
            return response;
        }
        if request_is_html_navigation {
            return Redirect::to("/login").into_response();
        }
    }
    AppError::new(kind).into_response()
}

/// Resolve the session cookie into a `Principal` (or the open local principal when
/// auth is not enforced) and enforce CSRF on every unsafe request.
pub async fn authenticate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let Some(auth) = state.auth().filter(|auth| auth.is_enforced()) else {
        if !is_safe_method(request.method()) && !same_origin(&request) {
            return AppError::new(ErrorKind::Forbidden).into_response();
        }
        let mut request = request;
        let path = request.uri().path();
        let public = is_anonymous_app_path(path);
        let principal = if state.local_mode() {
            crate::auth::Principal::local_admin()
        } else if public && state.is_configured() {
            crate::auth::Principal::public_connections(
                state
                    .profiles()
                    .iter()
                    .map(|profile| (profile.id.clone(), profile.managed && profile.id != "demo")),
            )
        } else if public {
            crate::auth::Principal::public_open()
        } else {
            crate::auth::Principal::open()
        };
        request.extensions_mut().insert(principal);
        return next.run(request).await;
    };
    let method = request.method().clone();
    let htmx = request.headers().contains_key("hx-request");
    let navigation = method == Method::GET && !htmx;
    let public = is_anonymous_app_path(&path);

    let has_session_cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(session::token_from_cookie_header)
        .is_some();
    let principal = match request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(session::token_from_cookie_header)
    {
        Some(token) => auth.authenticate(token).await,
        None => None,
    };

    if !is_safe_method(&method) && !same_origin(&request) {
        auth.record(
            AuditEvent::new(kind::ACCESS_DENIED, audit::DENIED)
                .resource(&path)
                .detail("cross-origin request"),
        )
        .await;
        return AppError::new(ErrorKind::Forbidden).into_response();
    }

    let Some(principal) = principal else {
        if public && (!has_session_cookie || path == "/login") {
            let mut request = request;
            let anonymous = crate::auth::Principal::public_connections(
                state
                    .profiles()
                    .into_iter()
                    .filter(|profile| profile.managed && profile.id != "demo")
                    .map(|profile| (profile.id, true)),
            );
            request.extensions_mut().insert(anonymous);
            return next.run(request).await;
        }
        return deny(
            navigation,
            htmx,
            if public && has_session_cookie && is_safe_method(&method) {
                ErrorKind::Forbidden
            } else {
                ErrorKind::Unauthorized
            },
        );
    };

    let mut request = request;
    let public_shell = public && !principal.is_admin();
    let principal = if public_shell {
        if principal.is_open() {
            principal
        } else {
            principal.with_public_connections(
                state
                    .profiles()
                    .into_iter()
                    .filter(|profile| profile.managed && profile.id != "demo")
                    .map(|profile| (profile.id, true)),
            )
        }
    } else {
        principal
    };
    if !is_safe_method(&method) {
        // CSRF: header, or a `csrf` form field. Buffer the (small) form body once.
        let (parts, body) = request.into_parts();
        let Ok(bytes) = to_bytes(body, MAX_FORM_BYTES).await else {
            return AppError::new(ErrorKind::PayloadTooLarge).into_response();
        };
        let supplied = parts
            .headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            .or_else(|| {
                url::form_urlencoded::parse(&bytes)
                    .find(|(name, _)| name == "csrf")
                    .map(|(_, value)| value.into_owned())
            })
            .unwrap_or_default();
        if path != "/login" && !session::secrets_equal(&supplied, principal.csrf()) {
            auth.record(
                AuditEvent::new(kind::ACCESS_DENIED, audit::DENIED)
                    .actor(principal.user_id(), principal.login())
                    .resource(&path)
                    .detail("csrf token missing or wrong"),
            )
            .await;
            return AppError::new(ErrorKind::Forbidden).into_response();
        }
        request = Request::from_parts(parts, Body::from(bytes));
    }
    request.extensions_mut().insert(principal);
    next.run(request).await
}

/// The authenticated (or open local) principal of this request.
pub struct Auth(pub Principal);

impl<S: Send + Sync> FromRequestParts<S> for Auth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .map(Auth)
            // The middleware always inserts one; its absence means it was bypassed: fail closed.
            .ok_or(AppError::new(ErrorKind::Unauthorized))
    }
}

impl Auth {
    /// Like `authorize`, for grants that must cover the whole profile (SQL, transactions).
    pub async fn authorize_wide(
        &self,
        state: &AppState,
        action: Action,
        profile: &str,
    ) -> Result<(), AppError> {
        let unconfigured_read = !state.is_configured()
            && self.0.is_open()
            && matches!(action, Action::View | Action::Export);
        if (unconfigured_read || self.0.allows_wide(action, profile, None))
            && (!self.0.is_open() || self.0.has_shared_connection_grants() || unconfigured_read)
        {
            return Ok(());
        }
        self.denied(state, action, &format!("{profile} (profile-wide)"))
            .await;
        Err(AppError::new(ErrorKind::Forbidden))
    }

    /// Deny by default. A denial is audited (once auth is enforced) and reported
    /// as a generic Forbidden so it reveals nothing about what exists.
    pub async fn authorize(
        &self,
        state: &AppState,
        action: Action,
        resource: &Resource<'_>,
    ) -> Result<(), AppError> {
        let unconfigured_read = !state.is_configured()
            && self.0.is_open()
            && matches!(action, Action::View | Action::Export);
        if (unconfigured_read || self.0.allows(action, resource))
            && (!self.0.is_open() || self.0.has_shared_connection_grants() || unconfigured_read)
        {
            return Ok(());
        }
        self.denied(state, action, &resource.describe()).await;
        Err(AppError::new(ErrorKind::Forbidden))
    }

    pub async fn denied(&self, state: &AppState, action: Action, what: &str) {
        if let Some(auth) = state.auth() {
            auth.record(
                AuditEvent::new(kind::ACCESS_DENIED, audit::DENIED)
                    .actor(self.0.user_id(), self.0.login())
                    .resource(what)
                    .detail(action.name()),
            )
            .await;
        }
    }

    pub async fn require_admin(&self, state: &AppState, what: &str) -> Result<(), AppError> {
        if self.0.is_admin() {
            return Ok(());
        }
        self.denied(state, Action::Edit, what).await;
        Err(AppError::new(ErrorKind::Forbidden))
    }
}

fn map_auth_error(error: AuthError) -> AppError {
    AppError::new(match error {
        AuthError::Invalid(_) | AuthError::Exists | AuthError::BootstrapRefused(_) => {
            ErrorKind::InvalidInput
        }
        AuthError::NotFound => ErrorKind::InvalidInput,
        AuthError::Denied | AuthError::Locked => ErrorKind::Forbidden,
        AuthError::Storage => ErrorKind::Internal,
    })
}

/// Policy refusals carry a short, static, user-correctable sentence ("password is too
/// simple"); show it instead of a generic failure. Everything else stays generic.
fn refused(error: AuthError) -> Result<Response, AppError> {
    match error {
        AuthError::Invalid(why) => notice_status(StatusCode::BAD_REQUEST, "refused", why),
        other => Err(map_auth_error(other)),
    }
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[derive(Template)]
#[template(path = "auth/login.html")]
struct LoginTemplate {
    message: String,
    csrf: String,
    title: &'static str,
}

#[derive(Template)]
#[template(path = "auth/notice.html")]
struct NoticeTemplate {
    kind: &'static str,
    message: String,
}

fn render<T: Template>(view: T) -> Result<Response, AppError> {
    view.render()
        .map(|body| no_store(Html(body).into_response()))
        .map_err(|_| AppError::template())
}

pub(super) fn notice(kind: &'static str, message: &str) -> Result<Response, AppError> {
    let mut response = render(NoticeTemplate {
        kind,
        message: message.to_string(),
    })?;
    if kind == "ok" {
        // Lets the page refresh the list that the change just altered (see app.js).
        response
            .headers_mut()
            .insert("hx-trigger", HeaderValue::from_static("cuadb:changed"));
    }
    Ok(response)
}

/// A notice fragment with an explicit status (policy refusals, conflicts).
pub(super) fn notice_status(
    status: StatusCode,
    kind: &'static str,
    message: &str,
) -> Result<Response, AppError> {
    let mut response = notice(kind, message)?;
    *response.status_mut() = status;
    Ok(response)
}

pub async fn login_form() -> Result<Response, AppError> {
    render(LoginTemplate {
        message: String::new(),
        csrf: String::new(),
        title: "Sign in · CuaDB",
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginForm {
    login: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<Response, AppError> {
    let Some(auth) = state.auth() else {
        return Err(AppError::new(ErrorKind::InvalidInput));
    };
    if form.password.len() > rbac_max_password() {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    match auth.login(&form.login, &form.password).await {
        Ok(outcome) => {
            let policy = auth.policy();
            let mut response = Redirect::to("/").into_response();
            if let Ok(value) = HeaderValue::from_str(&session::set_cookie(
                &outcome.token,
                policy.max_secs,
                policy.cookie_secure,
            )) {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
            Ok(no_store(response))
        }
        Err(AuthError::Locked) => {
            let mut response = render(LoginTemplate {
                message: "Too many attempts. Try again later.".into(),
                csrf: String::new(),
                title: "Sign in · CuaDB",
            })?;
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            Ok(response)
        }
        Err(_) => {
            let mut response = render(LoginTemplate {
                message: "Sign-in failed.".into(),
                csrf: String::new(),
                title: "Sign in · CuaDB",
            })?;
            *response.status_mut() = StatusCode::UNAUTHORIZED;
            Ok(response)
        }
    }
}

fn rbac_max_password() -> usize {
    crate::auth::password::MAX_PASSWORD_BYTES
}

pub async fn logout(State(state): State<AppState>, auth: Auth) -> Result<Response, AppError> {
    if let Some(service) = state.auth().filter(|_| !auth.0.is_open()) {
        service.logout(&auth.0).await;
        if let Some(id) = auth.0.user_id() {
            state.txs().rollback_user(id).await;
        }
        let mut response = Redirect::to("/login").into_response();
        if let Ok(value) =
            HeaderValue::from_str(&session::clear_cookie(service.policy().cookie_secure))
        {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
        return Ok(no_store(response));
    }
    Ok(Redirect::to("/").into_response())
}

fn service(state: &AppState) -> Result<&crate::auth::AuthService, AppError> {
    state.auth().ok_or(AppError::new(ErrorKind::InvalidInput))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordForm {
    current: String,
    new_password: String,
    #[serde(default)]
    csrf: String,
}

/// Change the signed-in user's own password.
pub async fn account_password(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<PasswordForm>,
) -> Result<Response, AppError> {
    let service = service(&state)?;
    let Some(id) = auth.0.user_id() else {
        return Err(AppError::new(ErrorKind::Forbidden));
    };
    let _ = form.csrf;
    service
        .change_password(&auth.0, id, Some(&form.current), &form.new_password)
        .await
        .map_err(map_auth_error)?;
    notice("ok", "Password changed. Other sessions were signed out.")
}

pub struct UserView {
    id: i64,
    login: String,
    role: String,
    status: &'static str,
}

#[derive(Template)]
#[template(path = "auth/users.html")]
struct UsersTemplate {
    users: Vec<UserView>,
    me: i64,
}

pub async fn admin_users(State(state): State<AppState>, auth: Auth) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/users").await?;
    let service = service(&state)?;
    let users = service
        .store
        .list_users()
        .await
        .map_err(map_auth_error)?
        .into_iter()
        .map(|u| UserView {
            id: u.id,
            login: u.login,
            role: u.role,
            status: if u.active { "active" } else { "disabled" },
        })
        .collect();
    render(UsersTemplate {
        users,
        me: auth.0.user_id().unwrap_or(0),
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUserForm {
    login: String,
    password: String,
    role: String,
    #[serde(default)]
    csrf: String,
}

pub async fn admin_create_user(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<CreateUserForm>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/users").await?;
    let _ = &form.csrf;
    let role = Role::parse(&form.role).map_err(map_auth_error)?;
    match service(&state)?
        .create_user(&auth.0, &form.login, &form.password, role)
        .await
    {
        Ok(_) => notice("ok", "Member created. Grant access under Permissions."),
        Err(error) => refused(error),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserStatusForm {
    user_id: i64,
    active: bool,
    #[serde(default)]
    csrf: String,
}

pub async fn admin_user_status(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<UserStatusForm>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/users/status").await?;
    let _ = &form.csrf;
    service(&state)?
        .set_user_active(&auth.0, form.user_id, form.active)
        .await
        .map_err(map_auth_error)?;
    if !form.active {
        state.txs().rollback_user(form.user_id).await;
    }
    notice("ok", "User status updated.")
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPasswordForm {
    user_id: i64,
    new_password: String,
    #[serde(default)]
    csrf: String,
}

pub async fn admin_user_password(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<UserPasswordForm>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/users/password").await?;
    let _ = &form.csrf;
    match service(&state)?
        .change_password(&auth.0, form.user_id, None, &form.new_password)
        .await
    {
        Ok(()) => notice("ok", "Password reset. The user was signed out everywhere."),
        Err(error) => refused(error),
    }
}

pub struct GrantView {
    id: i64,
    profile: String,
    schema: String,
    relation: String,
    actions: String,
}

#[derive(Template)]
#[template(path = "auth/grants.html")]
struct GrantsTemplate {
    user_id: i64,
    users: Vec<UserOption>,
    profiles: Vec<(String, String)>,
    grants: Vec<GrantView>,
}

pub struct UserOption {
    id: i64,
    login: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantsQuery {
    user: i64,
}

pub async fn admin_grants(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<GrantsQuery>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/grants").await?;
    let grants: Vec<Grant> = service(&state)?
        .grants(query.user)
        .await
        .map_err(map_auth_error)?;
    let users = service(&state)?
        .store
        .list_users()
        .await
        .map_err(map_auth_error)?
        .into_iter()
        .map(|u| UserOption {
            id: u.id,
            login: u.login,
        })
        .collect();
    let profiles = state
        .profiles()
        .into_iter()
        .map(|p| (p.id, p.label))
        .collect();
    render(GrantsTemplate {
        user_id: query.user,
        users,
        profiles,
        grants: grants
            .into_iter()
            .map(|g| GrantView {
                id: g.id,
                profile: g.profile,
                schema: g.schema,
                relation: g.relation,
                actions: rbac::action_names(g.actions),
            })
            .collect(),
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantForm {
    user_id: i64,
    profile: String,
    #[serde(default)]
    schema: String,
    #[serde(default)]
    relation: String,
    actions: String,
    #[serde(default)]
    csrf: String,
}

pub async fn admin_grant_add(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<GrantForm>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/grants").await?;
    let _ = &form.csrf;
    let actions = rbac::parse_actions(&form.actions).map_err(map_auth_error)?;
    // Grants may only name profiles that exist on this server.
    if !state.profiles().iter().any(|p| p.id == form.profile) && !state.profiles().is_empty() {
        return Err(AppError::new(ErrorKind::UnknownProfile));
    }
    match service(&state)?
        .add_grant(
            &auth.0,
            form.user_id,
            &form.profile,
            &form.schema,
            &form.relation,
            actions,
        )
        .await
    {
        Ok(_) => notice("ok", "Grant saved."),
        Err(error) => refused(error),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantRemoveForm {
    grant_id: i64,
    #[serde(default)]
    csrf: String,
}

pub async fn admin_grant_remove(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<GrantRemoveForm>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/grants/remove").await?;
    let _ = &form.csrf;
    service(&state)?
        .remove_grant(&auth.0, form.grant_id)
        .await
        .map_err(map_auth_error)?;
    notice("ok", "Grant removed.")
}

#[derive(Template)]
#[template(path = "auth/audit.html")]
struct AuditTemplate {
    events: Vec<StoredEvent>,
    next_before: Option<i64>,
    kinds: &'static [&'static str],
    actor: String,
    kind: String,
    outcome: String,
    since: String,
    until: String,
    /// Query string that repeats the filters for the "Older events" link.
    filter_query: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditQuery {
    limit: Option<i64>,
    before: Option<i64>,
    #[serde(default)]
    actor: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    since: String,
    #[serde(default)]
    until: String,
}

/// `YYYY-MM-DD` (UTC midnight) to Unix seconds; `None` for anything else.
fn day_start(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let (y, m, d): (i64, i64, i64) = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    if parts.next().is_some()
        || !(1970..=2200).contains(&y)
        || !(1..=12).contains(&m)
        || !(1..=31).contains(&d)
    {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400)
}

pub async fn admin_audit(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<AuditQuery>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/audit").await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let pick =
        |value: &str| (!value.is_empty()).then(|| value.chars().take(64).collect::<String>());
    // Only the closed vocabulary is accepted as a kind or outcome.
    let kind = pick(&query.kind).filter(|k| audit::KINDS.contains(&k.as_str()));
    let outcome = pick(&query.outcome)
        .filter(|o| [audit::OK, audit::DENIED, audit::FAILED].contains(&o.as_str()));
    let since = day_start(&query.since);
    // `until` is inclusive of the whole day.
    let until = day_start(&query.until).map(|t| t + 86_400);
    let filter = crate::auth::store::AuditFilter {
        actor: pick(&query.actor),
        kind: kind.clone(),
        outcome: outcome.clone(),
        since,
        until,
    };
    let events = service(&state)?
        .audit_events_filtered(&filter, limit, query.before)
        .await
        .map_err(map_auth_error)?;
    let next_before = (events.len() as i64 == limit)
        .then(|| events.last().map(|e| e.id))
        .flatten();
    let mut repeat = url::form_urlencoded::Serializer::new(String::new());
    if let Some(a) = &filter.actor {
        repeat.append_pair("actor", a);
    }
    if let Some(k) = &kind {
        repeat.append_pair("kind", k);
    }
    if let Some(o) = &outcome {
        repeat.append_pair("outcome", o);
    }
    if since.is_some() {
        repeat.append_pair("since", &query.since);
    }
    if until.is_some() {
        repeat.append_pair("until", &query.until);
    }
    render(AuditTemplate {
        events,
        next_before,
        kinds: audit::KINDS,
        actor: filter.actor.clone().unwrap_or_default(),
        kind: kind.unwrap_or_default(),
        outcome: outcome.unwrap_or_default(),
        since: if since.is_some() {
            query.since.clone()
        } else {
            String::new()
        },
        until: if until.is_some() {
            query.until.clone()
        } else {
            String::new()
        },
        filter_query: repeat.finish(),
    })
}

#[derive(Template)]
#[template(path = "auth/admin.html")]
struct AdminTemplate;

/// The Administration panel (tabs load their own fragments).
pub async fn admin_view(State(state): State<AppState>, auth: Auth) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin").await?;
    render(AdminTemplate)
}

#[derive(Template)]
#[template(path = "auth/security.html")]
struct SecurityTemplate {
    mode: &'static str,
    idle_minutes: i64,
    max_hours: i64,
    secure_cookie: bool,
    sql_workspace: bool,
    profiles: usize,
}

/// Read-only operator configuration. Nothing here can be changed from the browser.
pub async fn admin_security(
    State(state): State<AppState>,
    auth: Auth,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/security").await?;
    let policy = service(&state)?.policy();
    render(SecurityTemplate {
        mode: if auth.0.is_open() {
            "Trusted local"
        } else {
            "Authenticated"
        },
        idle_minutes: policy.idle_secs / 60,
        max_hours: policy.max_secs / 3600,
        secure_cookie: policy.cookie_secure,
        sql_workspace: state.sql_settings().enabled,
        profiles: state.profiles().len(),
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserRoleForm {
    user_id: i64,
    role: String,
    #[serde(default)]
    csrf: String,
}

/// POST /admin/users/role
pub async fn admin_user_role(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<UserRoleForm>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/users/role").await?;
    let _ = &form.csrf;
    let role = Role::parse(&form.role).map_err(map_auth_error)?;
    match service(&state)?
        .set_user_role(&auth.0, form.user_id, role)
        .await
    {
        Ok(()) => notice("ok", "Role updated."),
        Err(error) => refused(error),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDeleteForm {
    user_id: i64,
    #[serde(default)]
    csrf: String,
}

/// POST /admin/users/delete
pub async fn admin_user_delete(
    State(state): State<AppState>,
    auth: Auth,
    Form(form): Form<UserDeleteForm>,
) -> Result<Response, AppError> {
    auth.require_admin(&state, "/admin/users/delete").await?;
    let _ = &form.csrf;
    match service(&state)?.delete_user(&auth.0, form.user_id).await {
        Ok(()) => {
            state.txs().rollback_user(form.user_id).await;
            notice("ok", "Member deleted.")
        }
        Err(error) => refused(error),
    }
}
