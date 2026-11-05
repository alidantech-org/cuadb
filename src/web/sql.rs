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

/// Transactions and cancellations belong to a user; the trusted local instance has one
/// built-in user with no database row.
fn session_user(state: &AppState, auth: &Auth) -> Result<i64, AppError> {
    auth.0
        .user_id()
        .or_else(|| state.local_mode().then_some(0))
        .ok_or(AppError::new(ErrorKind::Forbidden))
}

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
    /// The exact connection id (a database switch keeps its own transactions apart).
    tx_key: String,
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
    let user = session_user(state, auth)?;
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
    let tx_key = profile_arg.clone().unwrap_or_else(|| pid.clone());
    if let Some(open) = state.txs().open_profile(user).await
        && open != tx_key
    {
        return Err(AppError::new(ErrorKind::Conflict));
    }
    Ok(Target {
        pid,
        tx_key,
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
    /// `numeric`, `bool-value`, `ts-value` or empty: the same classes the table explorer uses.
    class: &'static str,
    /// For booleans: whether the value is true.
    yes: bool,
}

pub struct ColView {
    name: String,
    ty: &'static str,
    numeric: bool,
}

fn is_timestamp(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
        && (b.len() == 10 || matches!(b[10], b' ' | b'T'))
}

/// Best-effort column type from the values the statement returned.
fn guess_type(rows: &[Vec<Cell>], index: usize) -> &'static str {
    let mut seen: Option<&'static str> = None;
    for cell in rows.iter().filter_map(|r| r.get(index)) {
        let kind = match cell {
            Cell::Null => continue,
            Cell::Integer(_) => "integer",
            Cell::Decimal(_) => "numeric",
            Cell::Boolean(_) => "boolean",
            Cell::Bytes(_) => "bytea",
            Cell::Text(t) if is_timestamp(t) => {
                if t.len() == 10 {
                    "date"
                } else {
                    "timestamptz"
                }
            }
            Cell::Text(t) if t.starts_with('{') || t.starts_with('[') => "json",
            _ => "text",
        };
        seen = match seen {
            None => Some(kind),
            Some(prev) if prev == kind => Some(prev),
            Some("integer") if kind == "numeric" => Some("numeric"),
            Some("numeric") if kind == "integer" => Some("numeric"),
            Some(_) => return "text",
        };
    }
    seen.unwrap_or("text")
}

fn view(cell: &Cell, ty: &str) -> CellView {
    let text = visible_text(&detail_value(cell, ""));
    let (class, yes) = match (cell, ty) {
        (Cell::Null, _) => ("", false),
        (Cell::Boolean(v), _) => ("bool-value", *v),
        (Cell::Integer(_) | Cell::Decimal(_), _) => ("numeric", false),
        (Cell::Text(_), "timestamptz" | "date") => ("ts-value", false),
        _ => ("", false),
    };
    CellView {
        is_null: matches!(cell, Cell::Null),
        text,
        class,
        yes,
    }
}

fn columns_and_rows(result: &crate::sql::exec::SqlResult) -> (Vec<ColView>, Vec<Vec<CellView>>) {
    let cols: Vec<ColView> = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let ty = guess_type(&result.rows, i);
            ColView {
                name: name.clone(),
                ty,
                numeric: matches!(ty, "integer" | "numeric"),
            }
        })
        .collect();
    let rows = result
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| view(cell, cols.get(i).map_or("text", |c| c.ty)))
                .collect()
        })
        .collect();
    (cols, rows)
}

fn render_result(
    kind: &'static str,
    summary: &str,
    result: crate::sql::exec::SqlResult,
) -> Result<Response, AppError> {
    let _ = summary;
    let step = step_ok(kind, result);
    let body = ScriptTemplate {
        status: "ok",
        headline: String::new(),
        steps: vec![step],
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

struct StepView {
    sql: String,
    kind: String,
    status: &'static str,
    meta: String,
    row_count: usize,
    truncated: bool,
    error: Option<String>,
    cols: Vec<ColView>,
    rows: Vec<Vec<CellView>>,
}

#[derive(Template)]
#[template(path = "sql/script_result.html")]
struct ScriptTemplate {
    status: &'static str,
    headline: String,
    steps: Vec<StepView>,
}

fn short(sql: &str) -> String {
    let one_line = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 160 {
        format!("{}…", one_line.chars().take(160).collect::<String>())
    } else {
        one_line
    }
}

fn statement_kind(sql: &str) -> String {
    sql.split_whitespace()
        .next()
        .unwrap_or("SQL")
        .trim_end_matches(';')
        .to_ascii_uppercase()
}

fn step_ok(sql: &str, result: crate::sql::exec::SqlResult) -> StepView {
    let meta = match result.affected {
        Some(n) => format!("{n} row(s) affected · {} ms", result.elapsed_ms),
        None => format!(
            "{} row(s) · {} ms{}",
            result.rows.len(),
            result.elapsed_ms,
            if result.truncated {
                " · truncated"
            } else {
                ""
            }
        ),
    };
    let (cols, rows) = columns_and_rows(&result);
    StepView {
        sql: short(sql),
        kind: statement_kind(sql),
        status: "ok",
        meta,
        row_count: rows.len(),
        truncated: result.truncated,
        error: None,
        cols,
        rows,
    }
}

fn step_note(sql: &str, meta: &str) -> StepView {
    StepView {
        sql: short(sql),
        kind: statement_kind(sql),
        status: "ok",
        meta: meta.to_string(),
        row_count: 0,
        truncated: false,
        error: None,
        cols: vec![],
        rows: vec![],
    }
}

fn step_err(sql: &str, message: String) -> StepView {
    StepView {
        sql: short(sql),
        kind: statement_kind(sql),
        status: "failed",
        meta: String::new(),
        row_count: 0,
        truncated: false,
        error: Some(message),
        cols: vec![],
        rows: vec![],
    }
}

/// Trusted local instance: any number of statements, any kind. `mode` is `tx` (one
/// transaction, all or nothing), `each` (one by one, stopping at the first error) or
/// `auto`; a script with its own BEGIN/COMMIT/ROLLBACK runs exactly as written.
async fn run_script(
    state: &AppState,
    auth: &Auth,
    pid: &str,
    workspace: std::sync::Arc<dyn Workspace>,
    sql: &str,
    mode: &str,
) -> Result<Response, AppError> {
    use crate::sql::script::{self, Control};
    let statements = script::split(sql);
    if statements.is_empty() {
        return notice_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "refused",
            "Enter some SQL to run.",
        );
    }
    let explicit = statements.iter().any(|s| script::control(s).is_some());
    let mode = if explicit {
        "explicit"
    } else {
        match mode {
            "each" => "each",
            "tx" => "tx",
            _ if statements.len() == 1 => "each",
            _ => "tx",
        }
    };
    let limits = limits(state);
    let max_rows = state.sql_settings().max_rows;
    let mut steps: Vec<StepView> = Vec::new();
    let mut tx: Option<Box<dyn crate::sql::exec::WriteTx>> = None;
    let mut open_explicit = false;
    let mut failed = false;
    let mut notes: Vec<String> = Vec::new();
    for statement in &statements {
        if tx.is_none() {
            match workspace.begin(&limits).await {
                Ok(new) => tx = Some(new),
                Err(error) => {
                    steps.push(step_err(statement, error.code().replace('_', " ")));
                    failed = true;
                    break;
                }
            }
        }
        let Some(current) = tx.as_mut() else { break };
        match script::control(statement) {
            Some(Control::Begin) => {
                open_explicit = true;
                steps.push(step_note(statement, "transaction started"));
                continue;
            }
            Some(Control::Commit) => {
                let result = tx.take().map(|t| t.commit());
                let outcome = match result {
                    Some(future) => future.await.map_err(|e| e.code().to_string()),
                    None => Ok(()),
                };
                open_explicit = false;
                match outcome {
                    Ok(()) => {
                        steps.push(step_note(statement, "committed"));
                    }
                    Err(message) => {
                        steps.push(step_err(statement, message));
                        failed = true;
                        break;
                    }
                }
                continue;
            }
            Some(Control::Rollback) => {
                if let Some(t) = tx.take() {
                    let _ = t.rollback().await;
                }
                open_explicit = false;
                steps.push(step_note(statement, "rolled back"));
                continue;
            }
            None => {}
        }
        match current.run_step(statement, max_rows).await {
            Ok(result) => {
                steps.push(step_ok(statement, result));
                let autocommit = match mode {
                    "each" => true,
                    "explicit" => !open_explicit,
                    _ => false,
                };
                if autocommit && let Some(t) = tx.take() {
                    match t.commit().await {
                        Ok(()) => {}
                        Err(error) => {
                            steps.push(step_err(statement, error.code().replace('_', " ")));
                            failed = true;
                            break;
                        }
                    }
                }
            }
            Err(message) => {
                steps.push(step_err(statement, message));
                failed = true;
                if let Some(t) = tx.take() {
                    let _ = t.rollback().await;
                }
                break;
            }
        }
    }
    if let Some(t) = tx.take() {
        if failed {
            let _ = t.rollback().await;
        } else if mode == "explicit" && open_explicit {
            let _ = t.rollback().await;
            notes
                .push("The script ended inside an open transaction, so it was rolled back.".into());
        } else {
            match t.commit().await {
                Ok(()) => {}
                Err(error) => {
                    steps.push(step_err("COMMIT", error.code().replace('_', " ")));
                    failed = true;
                }
            }
        }
    }
    let total = statements.len();
    let (status, headline) = if failed {
        let done = steps.iter().filter(|s| s.status == "ok").count();
        let tail = match mode {
            "tx" => "nothing was changed (rolled back)".to_string(),
            "explicit" => "the open transaction was rolled back".to_string(),
            _ => format!("{done} statement(s) before it stay applied"),
        };
        (
            "failed",
            format!("Stopped at statement {} of {total}: {tail}.", steps.len()),
        )
    } else if mode == "tx" {
        (
            "ok",
            format!("{total} statement(s) ran as one transaction and were committed."),
        )
    } else {
        ("ok", format!("{total} statement(s) ran."))
    };
    let headline = if notes.is_empty() {
        headline
    } else {
        format!("{headline} {}", notes.join(" "))
    };
    record(
        state,
        auth,
        AuditEvent::new(
            kind::SQL_EXECUTE,
            if failed { audit::FAILED } else { audit::OK },
        )
        .profile(pid)
        .resource("script")
        .detail(&format!("{total} statements, mode {mode}")),
    )
    .await;
    let ddl = statements.iter().any(|s| {
        let first = s
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            first.as_str(),
            "create" | "alter" | "drop" | "truncate" | "rename"
        )
    });
    let body = ScriptTemplate {
        status,
        headline,
        steps,
    }
    .render()
    .map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    if ddl && !failed {
        response.headers_mut().insert(
            "hx-trigger",
            axum::http::HeaderValue::from_static("cuadb:schema-changed"),
        );
    }
    Ok(response)
}

/// Trusted local instance, scripts that change data or schema: everything runs inside
/// one transaction that stays open for review. The pending bar then offers Commit and
/// Roll back; later runs join the same transaction until it ends.
#[allow(clippy::too_many_arguments)]
async fn run_script_tx(
    state: &AppState,
    auth: &Auth,
    pid: &str,
    workspace: std::sync::Arc<dyn Workspace>,
    user: i64,
    tx_key: &str,
    sql: &str,
    open_id: Option<&str>,
) -> Result<Response, AppError> {
    use crate::sql::exec::PendingChange;
    use crate::sql::script::{self, Control};
    let statements = script::split(sql);
    if statements.is_empty() {
        return notice_status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "refused",
            "Enter some SQL to run.",
        );
    }
    let limits = limits(state);
    let max_rows = state.sql_settings().max_rows;
    let ttl = std::time::Duration::from_secs(900).max(state.sql_settings().tx_ttl);
    let session = auth.0.0.session_hash.clone();
    // The transaction that is open now (if the caller has one), checked out for this run.
    let mut current: Option<(String, Box<dyn crate::sql::exec::WriteTx>)> = None;
    if let Some(id) = open_id {
        // A transaction that expired or was ended elsewhere is simply replaced by a new one.
        if let Ok((tx, profile)) = state.txs().checkout(id, user).await {
            if profile != tx_key {
                let _ = state.txs().restore(id, tx, None).await;
                return Err(AppError::new(ErrorKind::Conflict));
            }
            current = Some((id.to_string(), tx));
        }
    }
    let mut steps: Vec<StepView> = Vec::new();
    let mut staged: Vec<PendingChange> = Vec::new();
    let mut failed = false;
    let mut ended: Option<&'static str> = None;
    for statement in &statements {
        if current.is_none() {
            let tx = workspace.begin(&limits).await.map_err(classified_error)?;
            let id = state
                .txs()
                .begin(user, &session, tx_key, ttl, tx)
                .await
                .map_err(tx_error)?;
            let (tx, _) = state.txs().checkout(&id, user).await.map_err(tx_error)?;
            current = Some((id, tx));
            staged.clear();
        }
        let Some((id, tx)) = current.take() else {
            break;
        };
        match script::control(statement) {
            Some(Control::Begin) => {
                steps.push(step_note(statement, "already inside a transaction"));
                current = Some((id, tx));
                continue;
            }
            Some(Control::Commit | Control::Rollback) => {
                let commit = script::control(statement) == Some(Control::Commit);
                let pending = staged.clone();
                let _ = state
                    .txs()
                    .restore_with(&id, tx, std::mem::take(&mut staged))
                    .await;
                if let Ok((_, tx)) = state.txs().take(&id, user).await {
                    let outcome = if commit {
                        tx.commit().await
                    } else {
                        tx.rollback().await
                    };
                    match outcome {
                        Ok(()) => {
                            steps.push(step_note(
                                statement,
                                if commit { "committed" } else { "rolled back" },
                            ));
                            ended = Some(if commit { "committed" } else { "rolled back" });
                        }
                        Err(error) => {
                            steps.push(step_err(statement, error.code().replace('_', " ")));
                            failed = true;
                            break;
                        }
                    }
                }
                let _ = pending;
                continue;
            }
            None => {}
        }
        let mut tx = tx;
        match tx.run_step(statement, max_rows).await {
            Ok(result) => {
                if script::is_write(statement) {
                    staged.push(PendingChange {
                        summary: script::mask_literals(statement),
                        affected: result.affected.unwrap_or(0),
                    });
                }
                steps.push(step_ok(statement, result));
                current = Some((id, tx));
            }
            Err(message) => {
                steps.push(step_err(statement, message));
                failed = true;
                let _ = tx.rollback().await;
                state.txs().discard(&id, user).await;
                break;
            }
        }
    }
    let total = statements.len();
    let changes_staged = staged.len();
    let mut open: Option<String> = None;
    if let Some((id, tx)) = current.take() {
        match state.txs().restore_with(&id, tx, staged).await {
            Ok(()) => open = Some(id),
            Err(error) => return Err(tx_error(error)),
        }
    }
    let (status, headline) = if failed {
        (
            "failed",
            format!(
                "Stopped at statement {} of {total}: the transaction was rolled back, nothing changed.",
                steps.len()
            ),
        )
    } else if open.is_some() {
        (
            "ok",
            format!(
                "{total} statement(s) ran in a transaction · {changes_staged} change(s) staged. Commit to keep them or Roll back to undo."
            ),
        )
    } else {
        (
            "ok",
            format!(
                "{total} statement(s) ran · transaction {}.",
                ended.unwrap_or("ended")
            ),
        )
    };
    record(
        state,
        auth,
        AuditEvent::new(
            kind::SQL_EXECUTE,
            if failed { audit::FAILED } else { audit::OK },
        )
        .profile(pid)
        .resource("script")
        .detail(&format!("{total} statements in a transaction")),
    )
    .await;
    let body = ScriptTemplate {
        status,
        headline,
        steps,
    }
    .render()
    .map_err(|_| AppError::template())?;
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    let trigger = match (&open, failed) {
        (Some(id), _) => Some(format!(r#"{{"cuadb:tx":{{"id":"{id}"}}}}"#)),
        (None, _) => Some(r#"{"cuadb:tx":{"id":""}}"#.to_string()),
    };
    if let Some(value) = trigger
        && let Ok(value) = axum::http::HeaderValue::from_str(&value)
    {
        response.headers_mut().insert("hx-trigger", value);
    }
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
    if state.local_mode()
        && let Some(workspace) = state
            .workspace_for(profile_arg.as_deref())
            .filter(|workspace| workspace.writable())
    {
        let statements = crate::sql::script::split(sql);
        let explicit = statements
            .iter()
            .any(|s| crate::sql::script::control(s).is_some());
        let writes = statements.iter().any(|s| crate::sql::script::is_write(s));
        if wants_tx.is_some() || (writes && !explicit) {
            let user = session_user(&state, &auth)?;
            let tx_key = profile_arg.clone().unwrap_or_else(|| pid.clone());
            return run_script_tx(&state, &auth, &pid, workspace, user, &tx_key, sql, wants_tx)
                .await;
        }
        // Reads only (any number of statements) run normally; a script with its own
        // BEGIN/COMMIT/ROLLBACK runs exactly as written.
        return run_script(&state, &auth, &pid, workspace, sql, "each").await;
    }
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
                state
                    .cancels()
                    .register(auth.0.user_id().or_else(|| state.local_mode().then_some(0)))
            } else {
                state
                    .cancels()
                    .register_with(
                        &statement_id,
                        auth.0.user_id().or_else(|| state.local_mode().then_some(0)),
                    )
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
    if state.cancels().cancel(
        id,
        auth.0.user_id().or_else(|| state.local_mode().then_some(0)),
    ) {
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
            &target.tx_key,
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
    let user = session_user(&state, &auth)?;
    let (profile, changes, remaining) = state
        .txs()
        .pending(&query.tx, user)
        .await
        .map_err(tx_error)?;
    auth.authorize_wide(
        &state,
        Action::Edit,
        &state.effective_profile(Some(&profile)),
    )
    .await?;
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
    let user = session_user(&state, &auth)?;
    let reviewed: usize = required(&form, "expected_changes")?
        .parse()
        .map_err(|_| AppError::new(ErrorKind::InvalidInput))?;
    let (profile, changes, _) = state.txs().pending(id, user).await.map_err(tx_error)?;
    auth.authorize_wide(
        &state,
        Action::Edit,
        &state.effective_profile(Some(&profile)),
    )
    .await?;
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
    let user = session_user(&state, &auth)?;
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
    local: bool,
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
        local: state.local_mode() && workspace.writable(),
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
    /// Single-column foreign key: the referenced relation and column (empty when none).
    lk_schema: String,
    lk_relation: String,
    lk_column: String,
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
        || crate::db::postgres::row_query::is_spatial_type(&native)
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
        lk_schema: String::new(),
        lk_relation: String::new(),
        lk_column: String::new(),
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
    let adapter = state.adapter(profile.as_deref())?;
    let mut targets: std::collections::HashMap<String, (RelationId, String)> =
        std::collections::HashMap::new();
    if let Ok(list) = adapter.list_constraints(&id).await {
        for constraint in list.items {
            if constraint.kind == crate::db::types::ConstraintKind::ForeignKey
                && constraint.columns.len() == 1
                && let (Some(column), Some(target), Some(target_column)) = (
                    constraint.columns.first(),
                    constraint.references.clone(),
                    constraint.referenced_columns.first(),
                )
                && auth.0.allows(
                    Action::View,
                    &Resource::relation(&pid, &target.namespace, &target.name),
                )
            {
                targets.insert(column.clone(), (target, target_column.clone()));
            }
        }
    }
    let body = EditorTemplate {
        insert,
        title: if insert {
            format!("New row in {}", query.relation)
        } else {
            format!("Edit {}", query.relation)
        },
        fields: details
            .into_iter()
            .map(|d| {
                let mut field = editor_field(d, insert);
                if matches!(field.kind, "text" | "long" | "number")
                    && let Some((target, column)) = targets.get(&field.name)
                {
                    field.lk_schema = target.namespace.clone();
                    field.lk_relation = target.name.clone();
                    field.lk_column = column.clone();
                }
                field
            })
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

// ---- GET /sql/hints: bounded catalog names for autocomplete and the query builder

/// Schemas, relations and columns (at most 80 relations, 60 columns each).
pub async fn hints(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<crate::web::catalog::ProfileQuery>,
) -> Result<Response, AppError> {
    enabled(&state)?;
    let profile = checked_profile(query.profile)?;
    let pid = state.effective_profile(profile.as_deref());
    auth.authorize_wide(&state, Action::Sql, &pid).await?;
    let adapter = state.adapter(profile.as_deref())?;
    let mut tables = Vec::new();
    'outer: for namespace in adapter
        .list_schemas()
        .await
        .map_err(classified_error)?
        .items
    {
        if namespace.name.starts_with("pg_") || namespace.name == "information_schema" {
            continue;
        }
        let relations = adapter
            .list_relations(&namespace.name)
            .await
            .map(|list| list.items)
            .unwrap_or_default();
        for relation in relations {
            if tables.len() >= 80 {
                break 'outer;
            }
            let columns = adapter
                .columns(&relation.id)
                .await
                .map(|columns| {
                    columns
                        .into_iter()
                        .take(60)
                        .map(|c| serde_json::json!({"n": c.name, "t": c.native_type}))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            tables.push(serde_json::json!({
                "s": namespace.name,
                "n": relation.id.name,
                "c": columns,
            }));
        }
    }
    let mut response = axum::Json(serde_json::json!({ "tables": tables })).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

// ---- AI agent metadata tools: full schema list, per-table description, and search.

#[derive(serde::Deserialize)]
pub struct AiQuery {
    #[serde(default)]
    profile: String,
    #[serde(default)]
    schema: String,
    #[serde(default)]
    relation: String,
    #[serde(default)]
    q: String,
}

async fn ai_adapter(
    state: &AppState,
    auth: &Auth,
    profile: &str,
) -> Result<std::sync::Arc<dyn crate::db::adapter::DatabaseAdapter>, AppError> {
    enabled(state)?;
    let profile = checked_profile((!profile.is_empty()).then(|| profile.to_string()))?;
    let pid = state.effective_profile(profile.as_deref());
    auth.authorize_wide(state, Action::Sql, &pid).await?;
    state.adapter(profile.as_deref())
}

fn json_response(value: serde_json::Value) -> Response {
    let mut response = axum::Json(value).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

fn kind_word(kind: crate::db::types::RelationKind) -> &'static str {
    use crate::db::types::RelationKind::*;
    match kind {
        Table | PartitionedTable | Partition => "table",
        View => "view",
        MaterializedView => "materialized view",
        ForeignTable => "foreign table",
        VirtualTable => "virtual table",
    }
}

fn user_schema(name: &str) -> bool {
    !(name.starts_with("pg_") || name == "information_schema")
}

/// GET /ai/schema — every schema with its relation names (no columns).
pub async fn ai_schema(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<AiQuery>,
) -> Result<Response, AppError> {
    let adapter = ai_adapter(&state, &auth, &query.profile).await?;
    let mut schemas = Vec::new();
    let mut total = 0usize;
    let mut truncated = false;
    for namespace in adapter
        .list_schemas()
        .await
        .map_err(classified_error)?
        .items
    {
        if !user_schema(&namespace.name) {
            continue;
        }
        let relations = adapter
            .list_relations(&namespace.name)
            .await
            .map(|list| list.items)
            .unwrap_or_default();
        let mut tables = Vec::new();
        for relation in relations {
            if total >= 3000 {
                truncated = true;
                break;
            }
            total += 1;
            tables.push(serde_json::json!({"n": relation.id.name, "k": kind_word(relation.kind)}));
        }
        schemas.push(serde_json::json!({"name": namespace.name, "tables": tables}));
    }
    Ok(json_response(
        serde_json::json!({ "schemas": schemas, "truncated": truncated }),
    ))
}

/// GET /ai/describe?schema=&relation= — columns, keys and foreign keys of one relation.
pub async fn ai_describe(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<AiQuery>,
) -> Result<Response, AppError> {
    let adapter = ai_adapter(&state, &auth, &query.profile).await?;
    for part in [&query.schema, &query.relation] {
        if part.is_empty() || part.len() > 128 || part.chars().any(char::is_control) {
            return Err(AppError::new(ErrorKind::InvalidInput));
        }
    }
    let id = RelationId {
        namespace: query.schema.clone(),
        name: query.relation.clone(),
    };
    let mut details = adapter
        .describe_columns(&id)
        .await
        .map_err(classified_error)?
        .items;
    details.sort_by_key(|d| d.position);
    let columns: Vec<_> = details
        .iter()
        .take(120)
        .map(|d| {
            serde_json::json!({
                "name": d.column.name,
                "type": d.column.native_type,
                "nullable": d.column.nullable,
                "primary_key": d.column.primary_key,
                "default": d.column.default_expression,
                "comment": d.comment,
            })
        })
        .collect();
    let mut foreign_keys = Vec::new();
    if let Ok(list) = adapter.list_constraints(&id).await {
        for constraint in list.items {
            if constraint.kind == crate::db::types::ConstraintKind::ForeignKey
                && let Some(target) = constraint.references
            {
                foreign_keys.push(serde_json::json!({
                    "columns": constraint.columns,
                    "references": format!(
                        "{}.{}({})",
                        target.namespace,
                        target.name,
                        constraint.referenced_columns.join(", ")
                    ),
                }));
            }
        }
    }
    Ok(json_response(serde_json::json!({
        "schema": id.namespace,
        "relation": id.name,
        "columns": columns,
        "foreign_keys": foreign_keys,
    })))
}

/// GET /ai/search?q= — relations and columns whose names contain the text (bounded).
pub async fn ai_search(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<AiQuery>,
) -> Result<Response, AppError> {
    let adapter = ai_adapter(&state, &auth, &query.profile).await?;
    let needle = query.q.trim().to_lowercase();
    if needle.is_empty() || needle.len() > 64 || needle.chars().any(char::is_control) {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut hits = Vec::new();
    let mut scanned = 0usize;
    'outer: for namespace in adapter
        .list_schemas()
        .await
        .map_err(classified_error)?
        .items
    {
        if !user_schema(&namespace.name) {
            continue;
        }
        let schema_hit = namespace.name.to_lowercase().contains(&needle);
        let relations = adapter
            .list_relations(&namespace.name)
            .await
            .map(|list| list.items)
            .unwrap_or_default();
        for relation in relations {
            if hits.len() >= 40 || std::time::Instant::now() > deadline {
                break 'outer;
            }
            let table_hit = relation.id.name.to_lowercase().contains(&needle);
            if table_hit || schema_hit {
                hits.push(serde_json::json!({
                    "schema": namespace.name, "table": relation.id.name,
                    "kind": kind_word(relation.kind), "match": if table_hit { "table name" } else { "schema name" },
                }));
            }
            if scanned < 400 {
                scanned += 1;
                if let Ok(columns) = adapter.columns(&relation.id).await {
                    for column in columns
                        .into_iter()
                        .filter(|c| c.name.to_lowercase().contains(&needle))
                        .take(4)
                    {
                        if hits.len() >= 40 {
                            break;
                        }
                        hits.push(serde_json::json!({
                            "schema": namespace.name, "table": relation.id.name,
                            "column": column.name, "type": column.native_type,
                            "match": "column name",
                        }));
                    }
                }
            }
        }
    }
    Ok(json_response(
        serde_json::json!({ "query": query.q, "hits": hits, "tables_scanned_for_columns": scanned }),
    ))
}

/// GET /ai/relations — every foreign-key edge between visible relations (bounded scan).
pub async fn ai_relations(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<AiQuery>,
) -> Result<Response, AppError> {
    let adapter = ai_adapter(&state, &auth, &query.profile).await?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let mut edges = Vec::new();
    let mut scanned = 0usize;
    let mut truncated = false;
    'outer: for namespace in adapter
        .list_schemas()
        .await
        .map_err(classified_error)?
        .items
    {
        if !user_schema(&namespace.name) {
            continue;
        }
        let relations = adapter
            .list_relations(&namespace.name)
            .await
            .map(|list| list.items)
            .unwrap_or_default();
        for relation in relations {
            if scanned >= 600 || std::time::Instant::now() > deadline {
                truncated = true;
                break 'outer;
            }
            scanned += 1;
            if let Ok(list) = adapter.list_constraints(&relation.id).await {
                for constraint in list.items {
                    if constraint.kind == crate::db::types::ConstraintKind::ForeignKey
                        && let Some(target) = constraint.references
                    {
                        edges.push(serde_json::json!({
                            "from": format!("{}.{}", relation.id.namespace, relation.id.name),
                            "cols": constraint.columns,
                            "to": format!("{}.{}", target.namespace, target.name),
                            "tcols": constraint.referenced_columns,
                        }));
                    }
                }
            }
        }
    }
    Ok(json_response(
        serde_json::json!({ "edges": edges, "scanned": scanned, "truncated": truncated }),
    ))
}
