//! SQL workspace and typed-edit endpoints. All of them are POST (plus one GET status),
//! require a signed-in user with the matching grant, the operator switch
//! `CUADB_SQL_WORKSPACE=enabled`, and CSRF (enforced by the auth middleware).

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use super::{
    auth::{Auth, notice, notice_status},
    rows::{classified, detail_value, visible_text},
};
use crate::{
    auth::{
        Action, Resource,
        audit::{self, AuditEvent, kind},
    },
    db::types::{Cell, DbError, DbErrorKind, RelationId},
    error::{AppError, ErrorKind},
    sql::{
        crud::{self, Op, Value},
        exec::{ExecLimits, PendingChange, TxError, Workspace},
        policy::{self, Class, PolicyError},
    },
    state::{AppState, checked_profile},
};

fn enabled(state: &AppState) -> Result<(), AppError> {
    if state.sql_settings().enabled {
        Ok(())
    } else {
        Err(AppError::new(ErrorKind::FeatureDisabled))
    }
}

fn pairs(body: &Bytes) -> Result<Vec<(String, String)>, AppError> {
    if body.len() > 262_144 {
        return Err(AppError::new(ErrorKind::PayloadTooLarge));
    }
    Ok(url::form_urlencoded::parse(body)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect())
}

fn one<'a>(pairs: &'a [(String, String)], name: &str) -> Result<Option<&'a str>, AppError> {
    let mut found = pairs.iter().filter(|(k, _)| k == name);
    let first = found.next();
    if found.next().is_some() {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    Ok(first.map(|(_, v)| v.as_str()))
}

fn required<'a>(pairs: &'a [(String, String)], name: &str) -> Result<&'a str, AppError> {
    one(pairs, name)?
        .filter(|v| !v.is_empty())
        .ok_or(AppError::new(ErrorKind::InvalidInput))
}

fn limits(state: &AppState) -> ExecLimits {
    let s = state.sql_settings();
    ExecLimits {
        timeout: s.query_timeout,
        max_rows: s.max_rows,
        ..ExecLimits::default()
    }
}

struct Target {
    pid: String,
    profile_arg: Option<String>,
    workspace: std::sync::Arc<dyn Workspace>,
    user: i64,
}

/// Resolve profile, workspace and user, and refuse switching profiles mid-transaction.
async fn target(
    state: &AppState,
    auth: &Auth,
    pairs: &[(String, String)],
    need_write: bool,
) -> Result<Target, AppError> {
    enabled(state)?;
    let profile_arg = checked_profile(one(pairs, "profile")?.map(str::to_string))?;
    let pid = state.effective_profile(profile_arg.as_deref());
    let user = auth
        .0
        .user_id()
        .ok_or(AppError::new(ErrorKind::Forbidden))
        .inspect_err(|_| {})?;
    let Some(workspace) = state.workspace_for(profile_arg.as_deref()) else {
        return Err(AppError::new(ErrorKind::FeatureDisabled));
    };
    if need_write {
        // The caller has already authorized the specific resource (relation or profile).
        if !workspace.writable() {
            auth.denied(
                state,
                Action::Edit,
                &format!("{pid} (profile is read-only)"),
            )
            .await;
            return Err(AppError::new(ErrorKind::Forbidden));
        }
    }
    if let Some(open) = state.txs().open_profile(user).await
        && open != pid
    {
        return Err(AppError::new(ErrorKind::Conflict));
    }
    Ok(Target {
        pid,
        profile_arg,
        workspace,
        user,
    })
}

async fn record(state: &AppState, auth: &Auth, event: AuditEvent) {
    if let Some(service) = state.auth() {
        service
            .record(event.actor(auth.0.user_id(), auth.0.login()))
            .await;
    }
}

fn tx_error(error: TxError) -> AppError {
    AppError::new(match error {
        TxError::NotFound | TxError::Expired => ErrorKind::Conflict,
        TxError::TooMany | TxError::OtherProfile | TxError::TooManyChanges => ErrorKind::Conflict,
    })
}

// ---- result rendering

pub struct CellView {
    text: String,
    is_null: bool,
}

#[derive(Template)]
#[template(path = "sql/result.html")]
struct ResultTemplate {
    kind: &'static str,
    summary: String,
    affected: Option<u64>,
    elapsed_ms: u64,
    truncated: bool,
    columns: Vec<String>,
    rows: Vec<Vec<CellView>>,
}

fn view(cell: &Cell) -> CellView {
    CellView {
        is_null: matches!(cell, Cell::Null),
        text: visible_text(&detail_value(cell, "")),
    }
}

fn render_result(
    kind: &'static str,
    summary: &str,
    result: crate::sql::exec::SqlResult,
) -> Result<Response, AppError> {
    let page = ResultTemplate {
        kind,
        summary: summary.to_string(),
        affected: result.affected,
        elapsed_ms: result.elapsed_ms,
        truncated: result.truncated,
        columns: result.columns,
        rows: result
            .rows
            .iter()
            .map(|r| r.iter().map(view).collect())
            .collect(),
    };
    let body = page.render().map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

// ---- POST /sql/execute

pub async fn execute(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    let form = pairs(&body)?;
    let sql = required(&form, "sql")?;
    let wants_tx = one(&form, "tx")?.filter(|v| !v.is_empty());
    // Authorize before anything else (profile-wide SQL grant).
    enabled(&state)?;
    let profile_arg = checked_profile(one(&form, "profile")?.map(str::to_string))?;
    let pid = state.effective_profile(profile_arg.as_deref());
    auth.authorize_wide(&state, Action::Sql, &pid).await?;
    let can_write = auth.0.allows_wide(Action::Edit, &pid, None);
    let target = target(&state, &auth, &form, false).await?;
    let workspace = target.workspace.clone();
    let allow_writes = workspace.writable() && can_write;

    let classified = match policy::classify(workspace.dialect(), sql, allow_writes) {
        Ok(c) => c,
        Err(error) => {
            let why = if error == PolicyError::WritesNotAllowed && !workspace.writable() {
                "this connection profile is read-only"
            } else {
                // Static, user-correctable sentence: never echoes the statement.
                match error {
                    PolicyError::WritesNotAllowed => {
                        "you do not have permission to change data on this profile"
                    }
                    other => return refuse(&state, &auth, &pid, other).await,
                }
            };
            record(
                &state,
                &auth,
                AuditEvent::new(kind::SQL_EXECUTE, audit::DENIED)
                    .profile(&pid)
                    .detail(why),
            )
            .await;
            return notice_status(StatusCode::FORBIDDEN, "denied", why);
        }
    };

    let limits = limits(&state);
    match classified.class {
        Class::Read => {
            let statement_id = one(&form, "statement_id")?.unwrap_or_default().to_string();
            let handle = if statement_id.is_empty() {
                state.cancels().register(auth.0.user_id())
            } else {
                state
                    .cancels()
                    .register_with(&statement_id, auth.0.user_id())
                    .map(|h| (statement_id.clone(), h))
            };
            let Some((id, handle)) = handle else {
                return Err(AppError::new(ErrorKind::Overloaded));
            };
            let outcome = match wants_tx {
                Some(tx_id) => run_in_tx_read(&state, &target, tx_id, sql, limits.max_rows).await,
                None => workspace
                    .query(sql, &limits, &handle)
                    .await
                    .map_err(classified_error),
            };
            state.cancels().finish(&id);
            finish_sql(&state, &auth, &pid, &classified, outcome, None).await
        }
        Class::Write => {
            let built = crud::Built {
                sql: sql.to_string(),
                binds: vec![],
                summary: classified.summary.clone(),
                kind: classified.kind,
            };
            let affected = write(
                &state,
                &auth,
                &target,
                wants_tx,
                built,
                None,
                state.sql_settings().max_rows as u64,
            )
            .await;
            let outcome = affected.map(|n| crate::sql::exec::SqlResult {
                columns: vec![],
                rows: vec![],
                truncated: false,
                affected: Some(n),
                elapsed_ms: 0,
            });
            finish_sql(&state, &auth, &pid, &classified, outcome, Some(&target.pid)).await
        }
    }
}

async fn refuse(
    state: &AppState,
    auth: &Auth,
    pid: &str,
    error: PolicyError,
) -> Result<Response, AppError> {
    record(
        state,
        auth,
        AuditEvent::new(kind::SQL_EXECUTE, audit::DENIED)
            .profile(pid)
            .detail(&error.to_string()),
    )
    .await;
    notice_status(
        StatusCode::UNPROCESSABLE_ENTITY,
        "refused",
        &error.to_string(),
    )
}

fn classified_error(error: DbError) -> AppError {
    classified(error)
}

async fn finish_sql(
    state: &AppState,
    auth: &Auth,
    pid: &str,
    classified: &policy::Classified,
    outcome: Result<crate::sql::exec::SqlResult, AppError>,
    _write_profile: Option<&str>,
) -> Result<Response, AppError> {
    let event = AuditEvent::new(kind::SQL_EXECUTE, audit::OK)
        .profile(pid)
        .resource(classified.kind)
        .detail(&classified.summary);
    match outcome {
        Ok(result) => {
            let rows = result.rows.len();
            record(
                state,
                auth,
                event.resource(&format!(
                    "{} rows={}",
                    classified.kind,
                    rows.max(result.affected.unwrap_or(0) as usize)
                )),
            )
            .await;
            render_result(classified.kind, &classified.summary, result)
        }
        Err(error) => {
            let outcome = if error.kind() == ErrorKind::Forbidden {
                audit::DENIED
            } else {
                audit::FAILED
            };
            let mut failed = AuditEvent::new(kind::SQL_EXECUTE, outcome)
                .profile(pid)
                .resource(classified.kind)
                .detail(&classified.summary);
            failed.detail = Some(format!("{} [{}]", classified.summary, error.kind().code()));
            record(state, auth, failed).await;
            Err(error)
        }
    }
}

async fn run_in_tx_read(
    state: &AppState,
    target: &Target,
    tx_id: &str,
    sql: &str,
    max_rows: usize,
) -> Result<crate::sql::exec::SqlResult, AppError> {
    let (mut tx, profile) = state
        .txs()
        .checkout(tx_id, target.user)
        .await
        .map_err(tx_error)?;
    if profile != target.pid {
        let _ = state.txs().restore(tx_id, tx, None).await;
        return Err(AppError::new(ErrorKind::Conflict));
    }
    let result = tx.query(sql, max_rows).await.map_err(classified_error);
    match result {
        Ok(ok) => {
            let _ = state.txs().restore(tx_id, tx, None).await;
            Ok(ok)
        }
        Err(error) => {
            // A failed statement poisons PostgreSQL transactions: roll back and drop it.
            let _ = tx.rollback().await;
            state.txs().discard(tx_id, target.user).await;
            Err(error)
        }
    }
}

/// Execute one write either inside the caller's explicit transaction or in its own.
/// `exact` requires exactly that many rows (typed edits); otherwise at most `max`.
async fn write(
    state: &AppState,
    auth: &Auth,
    target: &Target,
    tx_id: Option<&str>,
    built: crud::Built,
    exact: Option<u64>,
    max: u64,
) -> Result<u64, AppError> {
    let _ = auth;
    let limits = limits(state);
    let check = |affected: u64| -> Result<u64, AppError> {
        match exact {
            Some(n) if affected != n => Err(AppError::new(ErrorKind::Conflict)),
            None if affected > max => Err(AppError::new(ErrorKind::Conflict)),
            _ => Ok(affected),
        }
    };
    match tx_id {
        Some(id) => {
            let (mut tx, profile) = state
                .txs()
                .checkout(id, target.user)
                .await
                .map_err(tx_error)?;
            if profile != target.pid {
                let _ = state.txs().restore(id, tx, None).await;
                return Err(AppError::new(ErrorKind::Conflict));
            }
            let outcome = match tx.execute(&built.sql, &built.binds).await {
                Ok(n) => check(n),
                Err(error) => Err(classified_error(error)),
            };
            match outcome {
                Ok(n) => {
                    state
                        .txs()
                        .restore(
                            id,
                            tx,
                            Some(PendingChange {
                                summary: built.summary.clone(),
                                affected: n,
                            }),
                        )
                        .await
                        .map_err(tx_error)?;
                    Ok(n)
                }
                Err(error) => {
                    // Validation or database failure: roll the whole transaction back.
                    let _ = tx.rollback().await;
                    state.txs().discard(id, target.user).await;
                    Err(error)
                }
            }
        }
        None => {
            let mut tx = target
                .workspace
                .begin(&limits)
                .await
                .map_err(classified_error)?;
            let result = match tx.execute(&built.sql, &built.binds).await {
                Ok(n) => check(n),
                Err(error) => Err(classified_error(error)),
            };
            match result {
                Ok(n) => {
                    tx.commit().await.map_err(|e| {
                        if e.kind == DbErrorKind::Conflict {
                            AppError::new(ErrorKind::Conflict)
                        } else {
                            classified_error(e)
                        }
                    })?;
                    Ok(n)
                }
                Err(error) => {
                    let _ = tx.rollback().await;
                    Err(error)
                }
            }
        }
    }
}

// ---- POST /sql/cancel

pub async fn cancel(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    enabled(&state)?;
    let form = pairs(&body)?;
    let id = required(&form, "statement_id")?;
    let pid = state
        .effective_profile(checked_profile(one(&form, "profile")?.map(str::to_string))?.as_deref());
    auth.authorize_wide(&state, Action::Sql, &pid).await?;
    if state.cancels().cancel(id, auth.0.user_id()) {
        notice("ok", "Cancellation requested.")
    } else {
        // Unknown, finished, or someone else's statement: indistinguishable.
        notice_status(
            StatusCode::NOT_FOUND,
            "missing",
            "No such running statement.",
        )
    }
}

// ---- typed edits

fn collect_values(
    form: &[(String, String)],
    prefix: &str,
    null_prefix: &str,
) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = vec![];
    for (name, value) in form {
        if let Some(column) = name.strip_prefix(prefix) {
            out.push((column.to_string(), Value::Text(value.clone())));
        } else if let Some(column) = name.strip_prefix(null_prefix) {
            out.push((column.to_string(), Value::Null));
        }
    }
    out
}

async fn edit(
    state: AppState,
    auth: Auth,
    body: Bytes,
    op_name: &'static str,
) -> Result<Response, AppError> {
    let form = pairs(&body)?;
    enabled(&state)?;
    let schema = required(&form, "schema")?;
    let relation = required(&form, "relation")?;
    for part in [schema, relation] {
        if part.len() > 128 || part.chars().any(char::is_control) {
            return Err(AppError::new(ErrorKind::InvalidInput));
        }
    }
    let profile_arg = checked_profile(one(&form, "profile")?.map(str::to_string))?;
    let pid = state.effective_profile(profile_arg.as_deref());
    auth.authorize(
        &state,
        Action::Edit,
        &Resource::relation(&pid, schema, relation),
    )
    .await?;
    let target = target(&state, &auth, &form, true).await?;
    let adapter = state.adapter(target.profile_arg.as_deref())?;
    let id = RelationId {
        namespace: schema.to_string(),
        name: relation.to_string(),
    };
    let details = adapter.describe_columns(&id).await.map_err(classified)?;
    let key: Vec<(String, String)> = form
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("key.").map(|c| (c.to_string(), v.clone())))
        .collect();
    let op = match op_name {
        "insert" => Op::Insert {
            values: collect_values(&form, "col.", "null."),
        },
        "update" => Op::Update {
            key,
            set: collect_values(&form, "col.", "null."),
            expected: collect_values(&form, "old.", "oldnull."),
        },
        _ => {
            // Deleting is destructive: the caller must repeat the relation name.
            if required(&form, "confirm")? != format!("{schema}.{relation}") {
                return notice_status(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "refused",
                    "Type schema.relation in `confirm` to delete.",
                );
            }
            Op::Delete { key }
        }
    };
    let built = match crud::build(
        target.workspace.dialect(),
        schema,
        relation,
        &details.items,
        &op,
    ) {
        Ok(b) => b,
        Err(error) => {
            record(
                &state,
                &auth,
                AuditEvent::new(kind::DATA_CHANGE, audit::FAILED)
                    .profile(&pid)
                    .resource(&format!("{schema}.{relation}"))
                    .detail(&format!("{op_name}: {error}")),
            )
            .await;
            return notice_status(
                StatusCode::UNPROCESSABLE_ENTITY,
                "refused",
                &error.to_string(),
            );
        }
    };
    let tx_id = one(&form, "tx")?.filter(|v| !v.is_empty());
    let summary = built.summary.clone();
    let outcome = write(&state, &auth, &target, tx_id, built, Some(1), 1).await;
    let base = AuditEvent::new(kind::DATA_CHANGE, audit::OK).profile(&pid);
    match outcome {
        Ok(n) => {
            record(
                &state,
                &auth,
                base.resource(&format!("{schema}.{relation}"))
                    .detail(&format!(
                        "{summary} rows={n}{}",
                        if tx_id.is_some() { " (pending)" } else { "" }
                    )),
            )
            .await;
            notice(
                "ok",
                &format!(
                    "{summary}: {n} row affected{}",
                    if tx_id.is_some() {
                        " (pending in transaction)"
                    } else {
                        ""
                    }
                ),
            )
        }
        Err(error) => {
            let outcome = if error.kind() == ErrorKind::Forbidden {
                audit::DENIED
            } else {
                audit::FAILED
            };
            record(
                &state,
                &auth,
                AuditEvent::new(kind::DATA_CHANGE, outcome)
                    .profile(&pid)
                    .resource(&format!("{schema}.{relation}"))
                    .detail(&format!("{summary} [{}]", error.kind().code())),
            )
            .await;
            Err(error)
        }
    }
}

pub async fn insert(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    edit(state, auth, body, "insert").await
}

pub async fn update(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    edit(state, auth, body, "update").await
}

pub async fn delete(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    edit(state, auth, body, "delete").await
}

// ---- transactions

#[derive(Template)]
#[template(path = "sql/tx.html")]
struct TxTemplate {
    tx: String,
    profile: String,
    remaining_secs: u64,
    changes: Vec<PendingChange>,
    total_affected: u64,
}

fn render_tx(
    tx: &str,
    profile: String,
    remaining: std::time::Duration,
    changes: Vec<PendingChange>,
) -> Result<Response, AppError> {
    let total_affected = changes.iter().map(|c| c.affected).sum();
    let body = TxTemplate {
        tx: tx.to_string(),
        profile,
        remaining_secs: remaining.as_secs(),
        changes,
        total_affected,
    }
    .render()
    .map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

pub async fn tx_begin(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    let form = pairs(&body)?;
    enabled(&state)?;
    let wanted = state
        .effective_profile(checked_profile(one(&form, "profile")?.map(str::to_string))?.as_deref());
    auth.authorize_wide(&state, Action::Edit, &wanted).await?;
    let target = target(&state, &auth, &form, true).await?;
    let session = auth.0.0.session_hash.clone();
    let tx = target
        .workspace
        .begin(&limits(&state))
        .await
        .map_err(classified_error)?;
    let id = state
        .txs()
        .begin(
            target.user,
            &session,
            &target.pid,
            state.sql_settings().tx_ttl,
            tx,
        )
        .await
        .map_err(tx_error)?;
    record(
        &state,
        &auth,
        AuditEvent::new(kind::TX_BEGIN, audit::OK).profile(&target.pid),
    )
    .await;
    render_tx(&id, target.pid, state.sql_settings().tx_ttl, vec![])
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TxQuery {
    tx: String,
}

pub async fn tx_status(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<TxQuery>,
) -> Result<Response, AppError> {
    enabled(&state)?;
    let user = auth
        .0
        .user_id()
        .ok_or(AppError::new(ErrorKind::Forbidden))?;
    let (profile, changes, remaining) = state
        .txs()
        .pending(&query.tx, user)
        .await
        .map_err(tx_error)?;
    auth.authorize_wide(&state, Action::Edit, &profile).await?;
    render_tx(&query.tx, profile, remaining, changes)
}

pub async fn tx_commit(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    enabled(&state)?;
    let form = pairs(&body)?;
    let id = required(&form, "tx")?;
    let user = auth
        .0
        .user_id()
        .ok_or(AppError::new(ErrorKind::Forbidden))?;
    let reviewed: usize = required(&form, "expected_changes")?
        .parse()
        .map_err(|_| AppError::new(ErrorKind::InvalidInput))?;
    let (profile, changes, _) = state.txs().pending(id, user).await.map_err(tx_error)?;
    auth.authorize_wide(&state, Action::Edit, &profile).await?;
    // The user must have reviewed exactly what is pending right now.
    if reviewed != changes.len() {
        return notice_status(
            StatusCode::CONFLICT,
            "refused",
            "The pending changes differ from what you reviewed; reload the transaction.",
        );
    }
    let (_, tx) = state.txs().take(id, user).await.map_err(tx_error)?;
    let total: u64 = changes.iter().map(|c| c.affected).sum();
    match tx.commit().await {
        Ok(()) => {
            record(
                &state,
                &auth,
                AuditEvent::new(kind::TX_COMMIT, audit::OK)
                    .profile(&profile)
                    .detail(&format!("changes={} rows={total}", changes.len())),
            )
            .await;
            notice(
                "ok",
                &format!("Committed {} change(s), {total} row(s).", changes.len()),
            )
        }
        Err(error) => {
            record(
                &state,
                &auth,
                AuditEvent::new(kind::TX_COMMIT, audit::FAILED)
                    .profile(&profile)
                    .detail(&format!("changes={} [{}]", changes.len(), error.code())),
            )
            .await;
            Err(AppError::new(if error.kind == DbErrorKind::Conflict {
                ErrorKind::Conflict
            } else {
                ErrorKind::Internal
            }))
        }
    }
}

pub async fn tx_rollback(
    State(state): State<AppState>,
    auth: Auth,
    body: Bytes,
) -> Result<Response, AppError> {
    enabled(&state)?;
    let form = pairs(&body)?;
    let id = required(&form, "tx")?;
    let user = auth
        .0
        .user_id()
        .ok_or(AppError::new(ErrorKind::Forbidden))?;
    let (entry, tx) = state.txs().take(id, user).await.map_err(tx_error)?;
    let _ = tx.rollback().await;
    record(
        &state,
        &auth,
        AuditEvent::new(kind::TX_ROLLBACK, audit::OK)
            .profile(&entry.profile)
            .detail(&format!("discarded={}", entry.pending.len())),
    )
    .await;
    notice("ok", "Transaction rolled back.")
}

// ---- GET /sql: the workspace panel

#[derive(Template)]
#[template(path = "sql/workspace.html")]
struct WorkspaceTemplate {
    profile_label: String,
    can_write: bool,
    max_rows: usize,
    timeout_secs: u64,
}

/// The editor page. It only renders controls; every statement is still classified and
/// authorized by `/sql/execute`.
pub async fn workspace_page(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<crate::web::catalog::ProfileQuery>,
) -> Result<Response, AppError> {
    enabled(&state)?;
    if auth.0.is_open() {
        // Trusted local mode has no identity to attribute statements to.
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let profile = checked_profile(query.profile)?;
    let pid = state.effective_profile(profile.as_deref());
    auth.authorize_wide(&state, Action::Sql, &pid).await?;
    let workspace = state
        .workspace_for(profile.as_deref())
        .ok_or(AppError::new(ErrorKind::FeatureDisabled))?;
    let label = state
        .profiles()
        .into_iter()
        .find(|p| p.id == pid)
        .map_or(pid.clone(), |p| p.label);
    let settings = state.sql_settings();
    let body = WorkspaceTemplate {
        profile_label: label,
        can_write: workspace.writable() && auth.0.allows_wide(Action::Edit, &pid, None),
        max_rows: settings.max_rows,
        timeout_secs: settings.query_timeout.as_secs(),
    }
    .render()
    .map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

// ---- GET /data/editor: the row form, rendered from catalog metadata

struct EditorField {
    name: String,
    native_type: String,
    /// `text`, `long`, `number`, `bool`, `enum`, or `readonly`.
    kind: &'static str,
    options: Vec<String>,
    pk: bool,
    nullable: bool,
    note: String,
}

#[derive(Template)]
#[template(path = "data/editor.html")]
struct EditorTemplate {
    insert: bool,
    title: String,
    fields: Vec<EditorField>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorQuery {
    schema: String,
    relation: String,
    mode: String,
    profile: Option<String>,
}

fn editor_field(detail: crate::db::types::ColumnDetail, insert: bool) -> EditorField {
    use crate::db::types::Availability;
    let column = detail.column;
    let native = column.native_type.to_ascii_lowercase();
    let mut notes: Vec<String> = Vec::new();
    if let Some(comment) = detail.comment.as_deref().filter(|c| !c.is_empty()) {
        notes.push(comment.chars().take(160).collect());
    }
    if let Some(default) = column.default_expression.as_deref() {
        notes.push(format!(
            "default {}",
            default.chars().take(60).collect::<String>()
        ));
    }
    let generated = detail.generated.is_some() || detail.identity.as_deref() == Some("ALWAYS");
    let unsupported = native.contains("json")
        || native.ends_with("[]")
        || native.contains("bytea")
        || native.contains("blob")
        || native.contains("binary")
        || native.starts_with("geometry")
        || native.starts_with("geography")
        || native.contains("tsvector")
        || native.contains("xml");
    let options = match &detail.enum_values {
        Availability::Available(values) => values.clone(),
        Availability::Unavailable => Vec::new(),
    };
    let kind = if generated {
        notes.insert(0, "generated by the database".into());
        "readonly"
    } else if unsupported {
        notes.insert(0, "not editable here".into());
        "readonly"
    } else if !insert && column.primary_key {
        notes.insert(0, "key: cannot be changed here".into());
        "readonly"
    } else if native == "boolean" || native == "bool" {
        "bool"
    } else if !options.is_empty() {
        "enum"
    } else if [
        "int", "numeric", "decimal", "real", "double", "float", "serial",
    ]
    .iter()
    .any(|word| native.contains(word))
    {
        "number"
    } else if native == "text" {
        "long"
    } else {
        "text"
    };
    EditorField {
        name: column.name,
        native_type: column.native_type,
        kind,
        options,
        pk: column.primary_key,
        nullable: column.nullable,
        note: notes.join(" · "),
    }
}

/// The insert/update form for one relation. It only describes controls; values are filled
/// in the browser from the row, and every change still goes through `/data/*`.
pub async fn editor(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<EditorQuery>,
) -> Result<Response, AppError> {
    enabled(&state)?;
    let insert = match query.mode.as_str() {
        "insert" => true,
        "update" => false,
        _ => return Err(AppError::new(ErrorKind::InvalidInput)),
    };
    for part in [&query.schema, &query.relation] {
        if part.is_empty() || part.len() > 128 || part.chars().any(char::is_control) {
            return Err(AppError::new(ErrorKind::InvalidInput));
        }
    }
    let profile = checked_profile(query.profile)?;
    let pid = state.effective_profile(profile.as_deref());
    auth.authorize(
        &state,
        Action::Edit,
        &Resource::relation(&pid, &query.schema, &query.relation),
    )
    .await?;
    let writable = state
        .workspace_for(profile.as_deref())
        .is_some_and(|workspace| workspace.writable());
    if auth.0.is_open() || !writable {
        return Err(AppError::new(ErrorKind::Forbidden));
    }
    let id = RelationId {
        namespace: query.schema.clone(),
        name: query.relation.clone(),
    };
    let mut details = state
        .adapter(profile.as_deref())?
        .describe_columns(&id)
        .await
        .map_err(classified)?
        .items;
    details.sort_by_key(|d| d.position);
    let body = EditorTemplate {
        insert,
        title: if insert {
            format!("New row in {}", query.relation)
        } else {
            format!("Edit {}", query.relation)
        },
        fields: details
            .into_iter()
            .map(|d| editor_field(d, insert))
            .collect(),
    }
    .render()
    .map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}
