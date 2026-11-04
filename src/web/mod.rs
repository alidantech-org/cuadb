//! HTML endpoints: full pages and reusable fragments rendered by Askama.

pub mod catalog;
pub mod export;
pub mod rows;

use askama::Template;
use axum::{
    extract::State,
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
};

use crate::{error::AppError, state::AppState};

#[derive(Template)]
#[template(path = "pages/explorer.html")]
struct ExplorerTemplate {
    title: &'static str,
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

pub async fn explorer() -> Result<Html<String>, AppError> {
    html(ExplorerTemplate {
        title: "CuaDB · Database explorer",
    })
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
    axum::extract::Query(query): axum::extract::Query<StatusQuery>,
) -> Result<Response, AppError> {
    let profile = crate::state::checked_profile(query.profile)?;
    let profile = profile.as_deref();
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
    id: String,
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
}

/// Trusted profiles configured on the server: ids, labels, engine family and
/// connection state. Never URLs, hosts, users or paths. The id is the only
/// thing the browser may send back (`profile=<id>`).
pub async fn connection_profiles(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<StatusQuery>,
) -> Result<Response, AppError> {
    let chosen = crate::state::checked_profile(query.profile)?;
    let infos = state.profiles();
    if let Some(id) = chosen.as_deref()
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
        let selected = match chosen.as_deref() {
            Some(id) => id == info.id,
            None => info.is_default,
        };
        profiles.push(ProfileRow {
            id: info.id,
            label: info.label,
            engine: info.engine,
            state: snapshot.map_or("Unavailable", |snapshot| snapshot.label()),
            ready: snapshot.is_some_and(|snapshot| snapshot.is_ready()),
            selected,
        });
    }
    let mut response = html(ProfileListTemplate { profiles })?.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
