//! HTML endpoints: full pages and reusable fragments rendered by Askama.

pub mod auth;
pub mod catalog;
pub mod connections;
pub mod export;
pub mod rows;
pub mod sql;

use askama::Template;
use axum::{
    extract::State,
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
};

use crate::{error::AppError, state::AppState};

/// What the page shell needs to know about the caller. Display and capability hints
/// only: every route still authorizes on the server.
pub struct Shell {
    pub login: String,
    pub role: &'static str,
    /// Trusted local mode: no administrator exists yet, so there is no session.
    pub local: bool,
    pub admin: bool,
    /// The SQL workspace is enabled by the operator and the caller is signed in.
    pub sql: bool,
}

impl Shell {
    pub fn new(state: &AppState, principal: &crate::auth::Principal) -> Self {
        let local = principal.is_open();
        Self {
            login: principal.login().to_string(),
            role: if local {
                "local"
            } else if principal.is_admin() {
                "admin"
            } else {
                "member"
            },
            local,
            admin: principal.is_admin(),
            sql: state.sql_settings().enabled
                && !local
                && state.profiles().iter().any(|profile| {
                    principal.allows_wide(crate::auth::Action::Sql, &profile.id, None)
                }),
        }
    }
}

#[derive(Template)]
#[template(path = "pages/explorer.html")]
struct ExplorerTemplate {
    title: &'static str,
    csrf: String,
    shell: Shell,
}

#[derive(Template)]
#[template(path = "components/status.html")]
struct StatusTemplate {
    heading: &'static str,
    detail: &'static str,
}

/// Template failures are classified but never exposed to the browser as raw
/// rendering errors, which may contain database-sourced values in future.
fn html<T: Template>(view: T) -> Result<Html<String>, AppError> {
    view.render().map(Html).map_err(|_| AppError::template())
}

pub async fn explorer(
    State(state): State<AppState>,
    auth: auth::Auth,
) -> Result<Response, AppError> {
    let shell = Shell::new(&state, &auth.0);
    let mut response = html(ExplorerTemplate {
        title: "CuaDB · Database explorer",
        csrf: auth.0.csrf().to_string(),
        shell,
    })?
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

/// Small, independently requested fragment proving HTMX wiring works.
pub async fn status() -> Result<Html<String>, AppError> {
    html(StatusTemplate {
        heading: "App shell ready",
        detail: "The server is responding. Read-only catalog inspection is available when connected.",
    })
}

/// Liveness only: never wait for the database or issue a SQL query here.
pub async fn health() -> &'static str {
    "ok"
}

#[derive(Template)]
#[template(path = "components/connection_status.html")]
struct ConnectionStatusTemplate {
    label: &'static str,
    detail: String,
    ready: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusQuery {
    profile: Option<String>,
}

/// Database status stays separate from /health and never renders credentials.
/// Failed probes are cached briefly, so repeated HTMX requests are inexpensive.
pub async fn connection_status(
    State(state): State<AppState>,
    auth: auth::Auth,
    axum::extract::Query(query): axum::extract::Query<StatusQuery>,
) -> Result<Response, AppError> {
    let profile = crate::state::checked_profile(query.profile)?;
    let profile = profile.as_deref();
    if state.is_configured() {
        let pid = state.effective_profile(profile);
        auth.authorize(
            &state,
            crate::auth::Action::View,
            &crate::auth::Resource::profile(&pid),
        )
        .await?;
    }
    // No profile named: the default profile, or "not configured" with no database.
    let snapshot = match profile {
        None => state.connection_snapshot().await,
        Some(_) => state
            .profile_snapshot(profile)
            .await
            .ok_or(AppError::new(crate::error::ErrorKind::UnknownProfile))?,
    };
    let detected_engine = state.profile_engine(profile).await;
    let view = ConnectionStatusTemplate {
        label: snapshot.label(),
        detail: snapshot.detail(detected_engine),
        ready: snapshot.is_ready(),
    };
    let mut response = html(view)?.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

struct ProfileRow {
    managed: bool,
    id: String,
    database: String,
    /// `read`, `write` (may edit) or `full` (may also run SQL).
    access: &'static str,
    label: String,
    engine: &'static str,
    state: &'static str,
    ready: bool,
    selected: bool,
}

#[derive(Template)]
#[template(path = "components/profile_list.html")]
struct ProfileListTemplate {
    profiles: Vec<ProfileRow>,
    admin: bool,
    full_admin: bool,
    local_setup: bool,
    key_set: bool,
}

/// Trusted profiles configured on the server: ids, labels, engine family and
/// connection state. Never URLs, hosts, users or paths. The id is the only
/// thing the browser may send back (`profile=<id>`).
pub async fn connection_profiles(
    State(state): State<AppState>,
    auth: auth::Auth,
    axum::extract::Query(query): axum::extract::Query<StatusQuery>,
) -> Result<Response, AppError> {
    let chosen = crate::state::checked_profile(query.profile)?;
    let chosen_base = chosen
        .as_deref()
        .map(|id| id.split('~').next().unwrap_or(id));
    // Profiles the user has no grant for are not even listed.
    let infos: Vec<_> = state
        .profiles()
        .into_iter()
        .filter(|info| auth.0.can_see_profile(&info.id))
        .collect();
    if let Some(id) = chosen_base
        && !infos.iter().any(|info| info.id == id)
    {
        return Err(AppError::new(crate::error::ErrorKind::UnknownProfile));
    }
    let mut probes = Vec::new();
    for info in &infos {
        let state = state.clone();
        let id = info.id.clone();
        probes.push(tokio::spawn(async move {
            state.profile_snapshot(Some(&id)).await
        }));
    }
    let mut profiles = Vec::new();
    for (info, probe) in infos.into_iter().zip(probes) {
        let snapshot = probe.await.ok().flatten();
        let selected = match chosen_base {
            Some(id) => id == info.id,
            None => info.is_default,
        };
        let access =
            if auth.0.allows_wide(crate::auth::Action::Sql, &info.id, None) && !auth.0.is_open() {
                "full"
            } else if auth
                .0
                .allows_wide(crate::auth::Action::Edit, &info.id, None)
                && !auth.0.is_open()
            {
                "write"
            } else {
                "read"
            };
        profiles.push(ProfileRow {
            managed: info.managed,
            access,
            database: info.database,
            id: info.id,
            label: info.label,
            engine: info.engine,
            state: snapshot.map_or("Unavailable", |snapshot| snapshot.label()),
            ready: snapshot.is_some_and(|snapshot| snapshot.is_ready()),
            selected,
        });
    }
    let mut response = html(ProfileListTemplate {
        profiles,
        admin: auth.0.is_admin() || (auth.0.is_open() && state.local_connection_management()),
        full_admin: auth.0.is_admin(),
        local_setup: auth.0.is_open() && !state.local_connection_management(),
        key_set: state.vault().is_some(),
    })?
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

struct DatabaseRow {
    name: String,
    current: bool,
}

#[derive(Template)]
#[template(path = "components/database_list.html")]
struct DatabaseListTemplate {
    databases: Vec<DatabaseRow>,
    base: String,
}

fn database_switch_allowed(auth: &auth::Auth, base: &str) -> bool {
    // Relation-scoped grants name tables, not databases, so only whole-connection access
    // (or the trusted local session) may move to another database of the server.
    auth.0.is_open() || auth.0.allows_wide(crate::auth::Action::View, base, None)
}

/// GET /connection/databases: databases of the selected connection's server.
pub async fn connection_databases(
    State(state): State<AppState>,
    auth: auth::Auth,
    axum::extract::Query(query): axum::extract::Query<StatusQuery>,
) -> Result<Response, AppError> {
    let requested = crate::state::checked_profile(query.profile)?;
    let base = state.effective_profile(requested.as_deref());
    if !database_switch_allowed(&auth, &base) {
        return Err(AppError::new(crate::error::ErrorKind::Forbidden));
    }
    let current = match requested.as_deref().and_then(crate::state::derived_parts) {
        Some((_, database)) => database,
        None => state
            .profiles()
            .into_iter()
            .find(|p| p.id == base)
            .map(|p| p.database)
            .unwrap_or_default(),
    };
    let names = state
        .databases_of(&base)
        .await
        .map_err(|_| AppError::new(crate::error::ErrorKind::InvalidInput))?;
    let databases = names
        .into_iter()
        .map(|name| DatabaseRow {
            current: name == current,
            name,
        })
        .collect();
    let mut response = html(DatabaseListTemplate { databases, base })?.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseDatabaseForm {
    profile: Option<String>,
    database: String,
    #[serde(default)]
    csrf: String,
}

/// POST /connection/use: switch to another database of the same server.
pub async fn connection_use(
    State(state): State<AppState>,
    auth: auth::Auth,
    axum::Form(form): axum::Form<UseDatabaseForm>,
) -> Result<Response, AppError> {
    let _ = &form.csrf;
    let requested = crate::state::checked_profile(form.profile)?;
    let base = state.effective_profile(requested.as_deref());
    if !database_switch_allowed(&auth, &base) {
        auth.denied(
            &state,
            crate::auth::Action::View,
            &format!("{base} (database switch)"),
        )
        .await;
        return Err(AppError::new(crate::error::ErrorKind::Forbidden));
    }
    if form.database.is_empty()
        || form.database.len() > 63
        || form.database.chars().any(char::is_control)
    {
        return Err(AppError::new(crate::error::ErrorKind::InvalidInput));
    }
    let id = state
        .use_database(&base, &form.database)
        .await
        .map_err(|_| AppError::new(crate::error::ErrorKind::InvalidInput))?;
    crate::auth::audit_profile_access(&state, &auth.0, &base).await;
    let body = format!(
        "<span id=\"use-result\" data-derived=\"{}\" data-db=\"{}\"></span>",
        id,
        form.database
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
    );
    let mut response = Html(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
