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
    problem(StatusCode::BAD_REQUEST, "Check the details", message, "")
}

/// Running inside a container (Docker, Podman or Kubernetes)?
fn in_docker() -> bool {
    std::path::Path::new("/.dockerenv").exists()
        || std::path::Path::new("/run/.containerenv").exists()
        || std::env::var_os("KUBERNETES_SERVICE_HOST").is_some()
}

/// A structured, copyable error: what failed, why, and what to try.
fn problem(
    status: StatusCode,
    title: &str,
    detail: &str,
    hint: &str,
) -> Result<Response, AppError> {
    let esc = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let html = format!(
        "<section class=\"notice error alert\" role=\"alert\" data-kind=\"refused\"><b>{}</b>{}{}</section>",
        esc(title),
        if detail.is_empty() {
            String::new()
        } else {
            format!("<p>{}</p>", esc(detail))
        },
        if hint.is_empty() {
            String::new()
        } else {
            format!("<small>{}</small>", esc(hint))
        }
    );
    let mut response = (status, axum::response::Html(html)).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

/// The URL with its password replaced by `****` (for showing a saved connection).
fn mask_url(raw: &str) -> String {
    match url::Url::parse(raw) {
        Ok(mut parsed) if parsed.password().is_some_and(|p| !p.is_empty()) => {
            let _ = parsed.set_password(Some("****"));
            parsed.into()
        }
        _ => raw.to_string(),
    }
}

async fn require_connection_manager(
    state: &AppState,
    auth: &Auth,
    route: &str,
) -> Result<(), AppError> {
    if auth.0.is_admin() || auth.0.is_public_connection_manager() {
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
    database: String,
}

#[derive(Template)]
#[template(path = "admin/connections.html")]
struct ListTemplate {
    rows: Vec<Row>,
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
            database: p.database,
        })
        .collect();
    render(ListTemplate {
        rows,
        can_manage: state.can_manage(),
    })
}

#[derive(Template)]
#[template(path = "admin/connection_form.html")]
struct FormTemplate {
    editing: bool,
    masked_url: String,
    has_password: bool,
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
            masked_url: String::new(),
            has_password: false,
            id: String::new(),
            label: String::new(),
            fields: Fields {
                engine: "postgresql".into(),
                port: "5432".into(),
                tls: "auto".into(),
                ..Fields::default()
            },
            writable: state.local_mode(),
        });
    };
    let (row, url) = stored(&state, &id).await?;
    let root = state.sqlite_root().unwrap_or_default();
    render(FormTemplate {
        editing: true,
        masked_url: mask_url(&url),
        has_password: url::Url::parse(&url)
            .is_ok_and(|u| u.password().is_some_and(|p| !p.is_empty())),
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
    url: String,
    #[serde(default)]
    save_after_test: String,
    #[serde(default)]
    writable: String,
    #[serde(default)]
    csrf: String,
}

impl ConnForm {
    fn fields(&self, saved_password: Option<String>) -> Fields {
        let credentials_supplied = !self.host.is_empty()
            || !self.user.is_empty()
            || !self.database.is_empty()
            || !self.password.is_empty();
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
            file: if self.engine == "postgresql" && !credentials_supplied {
                self.url.clone()
            } else {
                self.file.clone()
            },
        }
    }
}

fn submitted_url(
    form: &ConnForm,
    root: &std::path::Path,
    saved_password: Option<String>,
) -> Result<String, &'static str> {
    if !form.url.trim().is_empty() {
        if form.engine != "postgresql" {
            return Err("Pasted URLs are currently supported for PostgreSQL");
        }
        if form.host.is_empty() && form.user.is_empty() && form.database.is_empty() {
            let mut parsed =
                url::Url::parse(form.url.trim()).map_err(|_| "Enter a valid PostgreSQL URL")?;
            if let Some(password) = saved_password.filter(|_| {
                parsed
                    .password()
                    .is_none_or(|p| p.is_empty() || p == "****")
            }) {
                parsed
                    .set_password(Some(&password))
                    .map_err(|_| "The saved password could not be reused")?;
            }
            crate::db::postgres::url::parse(parsed.as_str())
                .map_err(|_| "The PostgreSQL URL is invalid or uses unsupported options")?;
            return Ok(parsed.into());
        }
        return Err("Use the pasted URL or the credential fields, not both");
    }
    connections::build_url(&form.fields(saved_password), root)
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

pub(super) async fn record(
    state: &AppState,
    auth: &Auth,
    id: &str,
    detail: &str,
    outcome: &'static str,
) {
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
            "These connection details are not valid: check the address, port, TLS mode or file name"
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
    let url = match submitted_url(&form, &root, None) {
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
    let mut response = notice("ok", "Connection saved and selected.")?;
    // Tells the page to refresh the lists and switch to the new connection (ids are slugs).
    if let Ok(value) = axum::http::HeaderValue::from_str(&format!(
        r#"{{"cuadb:changed":{{}},"cuadb:connection-added":{{"id":"{id}"}}}}"#
    )) {
        response.headers_mut().insert("hx-trigger", value);
    }
    Ok(response)
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
    let url = match submitted_url(&form, &root, connections::saved_password(&old_url)) {
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
    let url = match submitted_url(&form, &root, saved) {
        Ok(url) => url,
        Err(message) => return bad(message),
    };
    match state.test_url(&url).await {
        Ok((snapshot, millis)) if snapshot.is_ready() => {
            if form.save_after_test == "on" {
                if form.id.is_empty() {
                    return create(State(state), auth, axum::Form(form)).await;
                }
                return update(State(state), auth, axum::Form(form)).await;
            }
            record(&state, &auth, &form.id, "test ok", audit::OK).await;
            notice(
                "info",
                &format!("Connected in {millis} ms. Nothing was saved yet."),
            )
        }
        Ok((snapshot, _)) => {
            record(&state, &auth, &form.id, "test failed", audit::FAILED).await;
            let (title, detail, hint) = match (snapshot.status, snapshot.error_code) {
                (crate::db::types::ConnectionStatus::AuthenticationFailed, _) => (
                    "Sign-in was refused",
                    "The server rejected the user name or password.",
                    "Check the user and password. In a URL, special characters in the password must be percent-encoded (for example @ becomes %40).",
                ),
                (crate::db::types::ConnectionStatus::PermissionDenied, _) => (
                    "This user may not read the database",
                    "The login worked, but the role cannot inspect the catalog.",
                    "Grant CONNECT on the database and USAGE on its schemas, or use a role that can.",
                ),
                (_, Some("database_timeout")) => (
                    "No answer in time",
                    "The host did not respond before the connection timeout.",
                    "The address or port may be wrong, a firewall may be dropping the traffic, or the database may be down or overloaded.",
                ),
                _ => (
                    "Could not connect",
                    "The server could not be reached, or it refused the login or the database name.",
                    "Check that the database is running and the host, port, user, password and database name are right. For TLS, Automatic works for localhost; use Off for a local server without TLS.",
                ),
            };
            let hint = if in_docker() {
                format!(
                    "{hint} CuaDB is running in a container, where \"localhost\" is the container itself: use host.docker.internal for a database on this computer (add --add-host=host.docker.internal:host-gateway on Linux), a LAN address for another machine, or start the container with --network host."
                )
            } else {
                hint.to_string()
            };
            problem(StatusCode::BAD_GATEWAY, title, detail, &hint)
        }
        Err(error) => bad(manage_message(error)),
    }
}

/// POST /connection/reconnect — refresh the selected connection's catalog after closing idle pools.
pub async fn reconnect(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(form): axum::Form<IdForm>,
) -> Result<Response, AppError> {
    let _ = &form.csrf;
    let profile = crate::state::checked_profile(Some(form.id.clone()))?;
    let id = profile
        .as_deref()
        .ok_or(AppError::new(ErrorKind::InvalidInput))?;
    auth.authorize(
        &state,
        crate::auth::Action::View,
        &crate::auth::Resource::profile(id),
    )
    .await?;
    state
        .reconnect(id)
        .await
        .map_err(|_| AppError::new(ErrorKind::InvalidInput))?;
    notice("ok", "Connection refreshed.")
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisconnectForm {
    profile: String,
}

pub async fn disconnect(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(form): axum::Form<DisconnectForm>,
) -> Result<Response, AppError> {
    let profile = crate::state::checked_profile(Some(form.profile))?;
    let id = profile
        .as_deref()
        .ok_or(AppError::new(ErrorKind::InvalidInput))?;
    auth.authorize(
        &state,
        crate::auth::Action::View,
        &crate::auth::Resource::profile(id),
    )
    .await?;
    state
        .disconnect_profile(id)
        .await
        .map_err(|_| AppError::new(ErrorKind::InvalidInput))?;
    Ok(Html("").into_response())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleForm {
    profile: String,
    operation: String,
    #[serde(default)]
    database: String,
    #[serde(default)]
    schema: String,
    #[serde(default)]
    relation: String,
    #[serde(default)]
    confirm: String,
}

/// POST /connection/lifecycle — permission-scoped PostgreSQL database/object DDL.
pub async fn lifecycle(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(form): axum::Form<LifecycleForm>,
) -> Result<Response, AppError> {
    let requested = crate::state::checked_profile(Some(form.profile))?;
    let profile_id = requested
        .as_deref()
        .ok_or(AppError::new(ErrorKind::InvalidInput))?;
    let base = state.effective_profile(Some(profile_id));
    auth.authorize(
        &state,
        crate::auth::Action::Sql,
        &crate::auth::Resource::profile(&base),
    )
    .await?;
    let (operation, expected, description) = match form.operation.as_str() {
        "create-database" => (
            crate::state::DatabaseOperation::CreateDatabase(&form.database),
            format!("CREATE {}", form.database),
            format!("Create database {}", form.database),
        ),
        "drop-database" => (
            crate::state::DatabaseOperation::DropDatabase(&form.database),
            form.database.clone(),
            format!("Drop database {}", form.database),
        ),
        "drop-schema" => (
            crate::state::DatabaseOperation::DropSchema(&form.schema),
            form.schema.clone(),
            format!("Drop schema {} and its objects", form.schema),
        ),
        "drop-table" => (
            crate::state::DatabaseOperation::DropTable(&form.schema, &form.relation),
            format!("{}.{}", form.schema, form.relation),
            format!("Drop table {}.{}", form.schema, form.relation),
        ),
        _ => return Err(AppError::new(ErrorKind::InvalidInput)),
    };
    if form.confirm != expected {
        return notice_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "refused",
            &format!("Confirmation text did not match. Expected: {expected}"),
        );
    }
    state
        .database_lifecycle(profile_id, operation)
        .await
        .map_err(|_| AppError::new(ErrorKind::InvalidInput))?;
    record(&state, &auth, profile_id, &description, audit::OK).await;
    notice("ok", &format!("{description} completed."))
}

#[derive(serde::Deserialize)]
pub struct RevealQuery {
    id: String,
}

/// GET /admin/connections/reveal?id= — the saved password and URL, only for an
/// administrator or the trusted local instance, and only when asked for.
pub async fn reveal(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<RevealQuery>,
) -> Result<Response, AppError> {
    require_connection_manager(&state, &auth, "/admin/connections/reveal").await?;
    if !(state.local_mode() || auth.0.is_admin()) {
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let (row, url) = stored(&state, &query.id).await?;
    record(&state, &auth, &row.id, "password revealed", audit::OK).await;
    let password = url::Url::parse(&url)
        .ok()
        .and_then(|u| u.password().map(str::to_string))
        .unwrap_or_default();
    let mut response =
        axum::Json(serde_json::json!({ "url": url, "password": password })).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}
