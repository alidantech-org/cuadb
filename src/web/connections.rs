//! Administration of saved connections: list, create, edit, test and remove.
//!
//! Everything here is administrator-only and CSRF-protected by the auth middleware.
//! Passwords are sealed with AES-256-GCM before they touch disk and are never rendered:
//! the edit form leaves the password blank and "blank" means "keep the saved one".

use askama::Template;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use super::auth::{Auth, notice, notice_status};
use crate::{
    auth::audit::{self, AuditEvent, kind},
    connections::{self, Engine, Fields},
    error::{AppError, ErrorKind},
    state::{AppState, ManageError, ManagedInput},
};

fn bad(message: &str) -> Result<Response, AppError> {
    notice_status(StatusCode::BAD_REQUEST, "refused", message)
}

async fn require_connection_manager(
    state: &AppState,
    auth: &Auth,
    route: &str,
) -> Result<(), AppError> {
    if auth.0.is_admin() || (auth.0.is_open() && state.local_connection_management()) {
        Ok(())
    } else {
        auth.require_admin(state, route).await
    }
}

fn render<T: Template>(view: T) -> Result<Response, AppError> {
    let body = view.render().map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

struct Row {
    id: String,
    label: String,
    engine: &'static str,
    managed: bool,
    is_default: bool,
}

#[derive(Template)]
#[template(path = "admin/connections.html")]
struct ListTemplate {
    rows: Vec<Row>,
    key_set: bool,
    can_manage: bool,
}

/// GET /admin/connections
pub async fn list(State(state): State<AppState>, auth: Auth) -> Result<Response, AppError> {
    require_connection_manager(&state, &auth, "/admin/connections").await?;
    let rows = state
        .profiles()
        .into_iter()
        .map(|p| Row {
            id: p.id,
            label: p.label,
            engine: p.engine,
            managed: p.managed,
            is_default: p.is_default,
        })
        .collect();
    render(ListTemplate {
        rows,
        key_set: state.vault().is_some(),
        can_manage: state.can_manage(),
    })
}

#[derive(Template)]
#[template(path = "admin/connection_form.html")]
struct FormTemplate {
    editing: bool,
    id: String,
    label: String,
    fields: Fields,
    writable: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormQuery {
    id: Option<String>,
}

/// GET /admin/connections/form[?id=] — the dialog content.
pub async fn form(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<FormQuery>,
) -> Result<Response, AppError> {
    require_connection_manager(&state, &auth, "/admin/connections/form").await?;
    let Some(id) = query.id.filter(|id| !id.is_empty()) else {
        return render(FormTemplate {
            editing: false,
            id: String::new(),
            label: String::new(),
            fields: Fields {
                engine: "postgresql".into(),
                port: "5432".into(),
                tls: "verify-full".into(),
                ..Fields::default()
            },
            writable: false,
        });
    };
    let (row, url) = stored(&state, &id).await?;
    let root = state.sqlite_root().unwrap_or_default();
    render(FormTemplate {
        editing: true,
        id: row.id,
        label: row.label,
        fields: connections::read_fields(&url, &root),
        writable: row.writable,
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnForm {
    #[serde(default)]
    id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    engine: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    port: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    database: String,
    #[serde(default)]
    tls: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    writable: String,
    #[serde(default)]
    csrf: String,
}

impl ConnForm {
    fn fields(&self, saved_password: Option<String>) -> Fields {
        Fields {
            engine: self.engine.clone(),
            host: self.host.clone(),
            port: self.port.clone(),
            user: self.user.clone(),
            password: if self.password.is_empty() {
                saved_password
            } else {
                Some(self.password.clone())
            },
            database: self.database.clone(),
            tls: self.tls.clone(),
            file: self.file.clone(),
        }
    }
}

/// The stored row and its decrypted URL; the URL never leaves this module's callers.
async fn stored(
    state: &AppState,
    id: &str,
) -> Result<(crate::auth::store::ConnectionRow, String), AppError> {
    let auth = state.auth().ok_or(AppError::new(ErrorKind::InvalidInput))?;
    let vault = state
        .vault()
        .ok_or(AppError::new(ErrorKind::InvalidInput))?;
    let row = auth
        .store
        .connection(id)
        .await
        .map_err(|_| AppError::new(ErrorKind::Internal))?
        .ok_or(AppError::new(ErrorKind::InvalidInput))?;
    let url = vault
        .open(&row.id, &row.url_enc)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or(AppError::new(ErrorKind::Internal))?;
    Ok((row, url))
}

async fn record(state: &AppState, auth: &Auth, id: &str, detail: &str, outcome: &'static str) {
    if let Some(service) = state.auth() {
        service
            .record(
                AuditEvent::new(kind::CONNECTION_CHANGED, outcome)
                    .actor(auth.0.user_id(), auth.0.login())
                    .profile(id)
                    .detail(detail),
            )
            .await;
    }
}

fn manage_message(error: ManageError) -> &'static str {
    match error {
        ManageError::Exists => "A connection with this name already exists",
        ManageError::TooMany => "The connection limit has been reached",
        ManageError::Invalid => {
            "The server rejected these connection details; check the address, TLS mode and file"
        }
        ManageError::Unknown => {
            "That connection is defined in the server configuration or no longer exists"
        }
        ManageError::Unavailable => "This server cannot manage connections",
    }
}

fn prerequisites(state: &AppState) -> Option<&'static str> {
    if !state.can_manage() {
        return Some("This server cannot manage connections");
    }
    if state.vault().is_none() {
        return Some(
            "Set CUADB_SECRET_KEY on the server (openssl rand -hex 32) before saving connections",
        );
    }
    None
}

fn seal(state: &AppState, id: &str, url: &str) -> Option<Vec<u8>> {
    state.vault()?.seal(id, url.as_bytes())
}

/// POST /admin/connections
pub async fn create(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(form): axum::Form<ConnForm>,
) -> Result<Response, AppError> {
    require_connection_manager(&state, &auth, "/admin/connections").await?;
    let _ = &form.csrf;
    if let Some(message) = prerequisites(&state) {
        return bad(message);
    }
    if !connections::valid_label(&form.label) {
        return bad("Enter a name of up to 48 characters without leading or trailing spaces");
    }
    let id = if form.id.is_empty() {
        connections::slug(&form.label)
    } else {
        form.id.clone()
    };
    if let Err(message) = connections::check_id(&id) {
        return bad(message);
    }
    let root = state.sqlite_root().unwrap_or_default();
    let url = match connections::build_url(&form.fields(None), &root) {
        Ok(url) => url,
        Err(message) => return bad(message),
    };
    let writable = form.writable == "on";
    let input = ManagedInput {
        id: id.clone(),
        label: form.label.clone(),
        url: url.clone(),
        writable,
    };
    if let Err(error) = state.add_managed(&input) {
        return bad(manage_message(error));
    }
    let engine = Engine::parse(&form.engine).map_or("unknown", Engine::code);
    let saved = match (seal(&state, &id, &url), state.auth()) {
        (Some(url_enc), Some(auth_service)) => auth_service
            .store
            .save_connection(&crate::auth::store::ConnectionRow {
                id: id.clone(),
                label: form.label.clone(),
                engine: engine.into(),
                url_enc,
                writable,
            })
            .await
            .is_ok(),
        _ => false,
    };
    if !saved {
        let _ = state.remove_managed(&id).await;
        record(&state, &auth, &id, "create failed", audit::FAILED).await;
        return Err(AppError::new(ErrorKind::Internal));
    }
    record(&state, &auth, &id, "created", audit::OK).await;
    notice("ok", "Connection saved. Grant access under Permissions.")
}

/// POST /admin/connections/update
pub async fn update(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(form): axum::Form<ConnForm>,
) -> Result<Response, AppError> {
    require_connection_manager(&state, &auth, "/admin/connections/update").await?;
    let _ = &form.csrf;
    if let Some(message) = prerequisites(&state) {
        return bad(message);
    }
    if !connections::valid_label(&form.label) {
        return bad("Enter a name of up to 48 characters without leading or trailing spaces");
    }
    if !state.is_managed(&form.id) {
        return bad(manage_message(ManageError::Unknown));
    }
    let (_, old_url) = stored(&state, &form.id).await?;
    let root = state.sqlite_root().unwrap_or_default();
    let url =
        match connections::build_url(&form.fields(connections::saved_password(&old_url)), &root) {
            Ok(url) => url,
            Err(message) => return bad(message),
        };
    let writable = form.writable == "on";
    let input = ManagedInput {
        id: form.id.clone(),
        label: form.label.clone(),
        url: url.clone(),
        writable,
    };
    if let Err(error) = state.replace_managed(&input).await {
        return bad(manage_message(error));
    }
    let engine = Engine::parse(&form.engine).map_or("unknown", Engine::code);
    let saved = match (seal(&state, &form.id, &url), state.auth()) {
        (Some(url_enc), Some(auth_service)) => auth_service
            .store
            .save_connection(&crate::auth::store::ConnectionRow {
                id: form.id.clone(),
                label: form.label.clone(),
                engine: engine.into(),
                url_enc,
                writable,
            })
            .await
            .is_ok(),
        _ => false,
    };
    if !saved {
        record(&state, &auth, &form.id, "update failed", audit::FAILED).await;
        return Err(AppError::new(ErrorKind::Internal));
    }
    record(&state, &auth, &form.id, "updated", audit::OK).await;
    notice("ok", "Connection updated.")
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdForm {
    id: String,
    #[serde(default)]
    csrf: String,
}

/// POST /admin/connections/delete
pub async fn delete(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(form): axum::Form<IdForm>,
) -> Result<Response, AppError> {
    require_connection_manager(&state, &auth, "/admin/connections/delete").await?;
    let _ = &form.csrf;
    if !state.is_managed(&form.id) {
        return bad(manage_message(ManageError::Unknown));
    }
    if state.profiles().len() <= 1 {
        return bad("Keep at least one connection");
    }
    // Transactions on this connection end first so nothing is left open on a closed pool.
    state.txs().rollback_profile(&form.id).await;
    if let Err(error) = state.remove_managed(&form.id).await {
        return bad(manage_message(error));
    }
    if let Some(service) = state.auth() {
        let _ = service.store.delete_connection(&form.id).await;
    }
    record(
        &state,
        &auth,
        &form.id,
        "removed with its grants",
        audit::OK,
    )
    .await;
    notice("ok", "Connection removed.")
}

/// POST /admin/connections/test — try the details in the form without saving them.
pub async fn test(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(form): axum::Form<ConnForm>,
) -> Result<Response, AppError> {
    require_connection_manager(&state, &auth, "/admin/connections/test").await?;
    let _ = &form.csrf;
    if let Some(message) = prerequisites(&state) {
        return bad(message);
    }
    let saved = if !form.id.is_empty() && state.is_managed(&form.id) {
        stored(&state, &form.id)
            .await
            .ok()
            .and_then(|(_, url)| connections::saved_password(&url))
    } else {
        None
    };
    let root = state.sqlite_root().unwrap_or_default();
    let url = match connections::build_url(&form.fields(saved), &root) {
        Ok(url) => url,
        Err(message) => return bad(message),
    };
    match state.test_url(&url).await {
        Ok((snapshot, millis)) if snapshot.is_ready() => {
            record(&state, &auth, &form.id, "test ok", audit::OK).await;
            notice(
                "info",
                &format!("Connected in {millis} ms. Nothing was saved yet."),
            )
        }
        Ok((snapshot, _)) => {
            record(&state, &auth, &form.id, "test failed", audit::FAILED).await;
            notice_status(
                StatusCode::BAD_GATEWAY,
                "refused",
                &format!("Could not connect: {}.", snapshot.label().to_lowercase()),
            )
        }
        Err(error) => bad(manage_message(error)),
    }
}
