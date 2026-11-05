//! Table and schema actions (rename, add column, drop, run SQL …). One dialog serves every
//! action: the server builds the statement (with safe identifier quoting), previews it, and
//! runs it only after the typed confirmation that destructive actions require.

use askama::Template;
use axum::{
    Json,
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
};
use serde_json::json;

use super::{auth::Auth, connections::record};
use crate::{
    auth::{Action, audit},
    db::types::{Engine, RelationId},
    error::{AppError, ErrorKind},
    state::{AppState, checked_profile},
};

struct Tab {
    id: &'static str,
    label: &'static str,
}

#[derive(Template)]
#[template(path = "components/object_actions.html")]
struct DialogTemplate {
    title: String,
    scope: String,
    schema: String,
    relation: String,
    database: String,
    profile: String,
    engine: &'static str,
    tabs: Vec<Tab>,
    columns: Vec<String>,
    local: bool,
    default_sql: String,
}

#[derive(serde::Deserialize)]
pub struct DialogQuery {
    #[serde(default)]
    profile: String,
    scope: String,
    #[serde(default)]
    schema: String,
    #[serde(default)]
    relation: String,
    #[serde(default)]
    database: String,
}

fn quote(name: &str) -> Result<String, &'static str> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err("Enter a valid name");
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}

fn qualified(schema: &str, name: &str) -> Result<String, &'static str> {
    Ok(format!("{}.{}", quote(schema)?, quote(name)?))
}

fn safe_fragment(text: &str, max: usize) -> bool {
    !text.is_empty()
        && text.len() <= max
        && !text.contains([';', '\0'])
        && !text.contains("--")
        && !text.contains("/*")
}

fn safe_type(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '(' | ')' | ',' | '[' | ']')
        })
}

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

async fn engine_of(state: &AppState, profile: Option<&str>) -> Result<Engine, AppError> {
    state
        .profile_engine(profile)
        .await
        .ok_or(AppError::new(ErrorKind::UnknownProfile))
}

fn tabs_for(scope: &str, engine: Engine, local: bool, has_database: bool) -> Vec<Tab> {
    let pg = engine == Engine::PostgreSql;
    let mut tabs = Vec::new();
    let mut add = |id, label| tabs.push(Tab { id, label });
    if scope == "table" {
        add("rename", "Rename");
        add("add-column", "Add column");
        add("rename-column", "Rename column");
        add("drop-column", "Drop column");
        if pg {
            add("comment", "Comment");
        }
        add("truncate", "Empty table");
        add("drop", "Drop");
    } else if scope == "database" {
        add("create-schema", "New schema");
        add("create-database", "New database");
        if has_database {
            add("drop-database", "Drop database");
        }
    } else {
        add("create-table", "Create table");
        if pg {
            add("rename", "Rename");
            add("comment", "Comment");
            add("drop", "Drop");
        }
    }
    if local && scope != "database" {
        add("sql", "Run SQL");
    }
    tabs
}

/// GET /object/actions — the dialog for one table or schema.
pub async fn dialog(
    State(state): State<AppState>,
    auth: Auth,
    Query(query): Query<DialogQuery>,
) -> Result<Response, AppError> {
    if !matches!(query.scope.as_str(), "table" | "schema" | "database") {
        return Err(AppError::new(ErrorKind::InvalidInput));
    }
    let profile = checked_profile((!query.profile.is_empty()).then(|| query.profile.clone()))?;
    let base = state.effective_profile(profile.as_deref());
    auth.authorize_wide(&state, Action::Sql, &base).await?;
    let engine = engine_of(&state, profile.as_deref()).await?;
    let local = state.local_mode();
    let mut columns = Vec::new();
    if query.scope == "table" {
        let id = RelationId {
            namespace: query.schema.clone(),
            name: query.relation.clone(),
        };
        if let Ok(adapter) = state.adapter(profile.as_deref())
            && let Ok(details) = adapter.describe_columns(&id).await
        {
            let mut items = details.items;
            items.sort_by_key(|d| d.position);
            columns = items.into_iter().map(|d| d.column.name).collect();
        }
    }
    let (title, default_sql) = if query.scope == "database" {
        (
            if query.database.is_empty() {
                "Databases".to_string()
            } else {
                format!("Database {}", query.database)
            },
            String::new(),
        )
    } else if query.scope == "table" {
        (
            format!("{}.{}", query.schema, query.relation),
            format!(
                "SELECT * FROM {} LIMIT 100",
                qualified(&query.schema, &query.relation).unwrap_or_default()
            ),
        )
    } else {
        (format!("Schema {}", query.schema), "SELECT 1".to_string())
    };
    let tabs = tabs_for(&query.scope, engine, local, !query.database.is_empty());
    let body = DialogTemplate {
        title,
        scope: query.scope,
        schema: query.schema,
        relation: query.relation,
        database: query.database,
        profile: profile.unwrap_or_default(),
        engine: if engine == Engine::Sqlite {
            "sqlite"
        } else {
            "postgresql"
        },
        tabs,
        columns,
        local,
        default_sql,
    }
    .render()
    .map_err(|_| AppError::template())?;
    Ok(Html(body).into_response())
}

struct Plan {
    statements: Vec<String>,
    destructive: bool,
    confirm: String,
    warning: String,
    /// Runs against the server's maintenance database, outside any transaction.
    server_level: bool,
}

fn plan(
    engine: Engine,
    form: &std::collections::HashMap<String, String>,
) -> Result<Plan, &'static str> {
    let get = |k: &str| form.get(k).map_or("", |v| v.trim());
    let scope = get("scope");
    let schema = get("schema");
    let relation = get("relation");
    let pg = engine == Engine::PostgreSql;
    let cascade = form.get("cascade").is_some_and(|v| v == "on");
    let table = || qualified(schema, relation);
    let mut plan = Plan {
        statements: vec![],
        destructive: false,
        confirm: String::new(),
        warning: String::new(),
        server_level: false,
    };
    let action = get("action");
    let sql = match (scope, action) {
        ("table", "rename") => {
            format!(
                "ALTER TABLE {} RENAME TO {}",
                table()?,
                quote(get("new_name"))?
            )
        }
        ("table", "add-column") => {
            let ty = get("col_type");
            if !safe_type(ty) {
                return Err("Enter a column type such as text, integer or timestamptz");
            }
            let mut s = format!(
                "ALTER TABLE {} ADD COLUMN {} {ty}",
                table()?,
                quote(get("col_name"))?
            );
            if form.get("col_notnull").is_some_and(|v| v == "on") {
                s.push_str(" NOT NULL");
            }
            let default = get("col_default");
            if !default.is_empty() {
                if !safe_fragment(default, 200) {
                    return Err("The default expression is not allowed");
                }
                s.push_str(&format!(" DEFAULT {default}"));
            }
            s
        }
        ("table", "rename-column") => format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            table()?,
            quote(get("column"))?,
            quote(get("new_name"))?
        ),
        ("table", "drop-column") => {
            plan.destructive = true;
            plan.confirm = get("column").to_string();
            plan.warning = "Dropping a column permanently deletes its data.".into();
            format!(
                "ALTER TABLE {} DROP COLUMN {}",
                table()?,
                quote(get("column"))?
            )
        }
        ("table", "comment") if pg => {
            format!(
                "COMMENT ON TABLE {} IS {}",
                table()?,
                literal(get("comment_text"))
            )
        }
        ("schema", "comment") if pg => {
            format!(
                "COMMENT ON SCHEMA {} IS {}",
                quote(schema)?,
                literal(get("comment_text"))
            )
        }
        ("table", "truncate") => {
            plan.destructive = true;
            plan.confirm = relation.to_string();
            plan.warning = "Every row in this table will be permanently deleted.".into();
            if pg {
                format!(
                    "TRUNCATE TABLE {}{}",
                    table()?,
                    if cascade { " CASCADE" } else { "" }
                )
            } else {
                format!("DELETE FROM {}", table()?)
            }
        }
        ("table", "drop") => {
            plan.destructive = true;
            plan.confirm = format!("{schema}.{relation}");
            plan.warning = "The table and all of its data will be permanently deleted.".into();
            format!(
                "DROP TABLE {}{}",
                table()?,
                if pg && cascade { " CASCADE" } else { "" }
            )
        }
        ("schema", "create-table") => {
            let defs = if get("col_defs").is_empty() {
                if pg {
                    "id bigserial PRIMARY KEY"
                } else {
                    "id INTEGER PRIMARY KEY AUTOINCREMENT"
                }
            } else {
                get("col_defs")
            };
            if !safe_fragment(defs, 1000) {
                return Err("The column definitions are not allowed");
            }
            format!(
                "CREATE TABLE {} ({defs})",
                qualified(schema, get("new_name"))?
            )
        }
        ("schema", "rename") if pg => {
            format!(
                "ALTER SCHEMA {} RENAME TO {}",
                quote(schema)?,
                quote(get("new_name"))?
            )
        }
        ("schema", "drop") if pg => {
            plan.destructive = true;
            plan.confirm = schema.to_string();
            plan.warning =
                "The schema and, with CASCADE, every object in it will be permanently deleted."
                    .into();
            format!(
                "DROP SCHEMA {}{}",
                quote(schema)?,
                if cascade { " CASCADE" } else { "" }
            )
        }
        ("database", "create-schema") if pg => {
            format!("CREATE SCHEMA {}", quote(get("new_name"))?)
        }
        ("database", "create-database") if pg => {
            plan.server_level = true;
            format!("CREATE DATABASE {}", quote(get("new_name"))?)
        }
        ("database", "drop-database") if pg => {
            plan.server_level = true;
            plan.destructive = true;
            plan.confirm = get("database").to_string();
            plan.warning =
                "The database and everything in it will be permanently deleted. Open connections to it are closed."
                    .into();
            format!("DROP DATABASE {} WITH (FORCE)", quote(get("database"))?)
        }
        (_, "sql") => {
            let text = get("sql");
            if text.is_empty() || text.len() > 20_000 {
                return Err("Enter some SQL to run");
            }
            plan.warning = "Runs exactly as written on this connection.".into();
            text.to_string()
        }
        _ => return Err("That action is not available here"),
    };
    plan.statements.push(sql);
    Ok(plan)
}

fn form_map(pairs: Vec<(String, String)>) -> std::collections::HashMap<String, String> {
    pairs.into_iter().collect()
}

async fn prepare(
    state: &AppState,
    auth: &Auth,
    form: &std::collections::HashMap<String, String>,
) -> Result<(Option<String>, String, Plan), Response> {
    let fail = |status: axum::http::StatusCode, message: &str| {
        (status, Json(json!({"ok": false, "error": message}))).into_response()
    };
    let profile = checked_profile(form.get("profile").filter(|p| !p.is_empty()).cloned())
        .map_err(|e| e.into_response())?;
    let base = state.effective_profile(profile.as_deref());
    auth.authorize_wide(state, Action::Sql, &base)
        .await
        .map_err(|e| e.into_response())?;
    let engine = engine_of(state, profile.as_deref())
        .await
        .map_err(|e| e.into_response())?;
    if form.get("action").is_some_and(|a| a == "sql") && !state.local_mode() {
        return Err(fail(
            axum::http::StatusCode::FORBIDDEN,
            "Free-form SQL actions are only available on a local instance",
        ));
    }
    let planned = plan(engine, form)
        .map_err(|message| fail(axum::http::StatusCode::UNPROCESSABLE_ENTITY, message))?;
    Ok((profile, base, planned))
}

/// POST /object/actions/preview — the statement that would run, never executed.
pub async fn preview(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(pairs): axum::Form<Vec<(String, String)>>,
) -> Response {
    let form = form_map(pairs);
    match prepare(&state, &auth, &form).await {
        Ok((_, _, planned)) => Json(json!({
            "ok": true,
            "sql": planned.statements.join(";\n") + ";",
            "destructive": planned.destructive,
            "confirm": planned.confirm,
            "warning": planned.warning,
        }))
        .into_response(),
        Err(response) => response,
    }
}

/// POST /object/actions/run — executes after the typed confirmation for destructive actions.
pub async fn run(
    State(state): State<AppState>,
    auth: Auth,
    axum::Form(pairs): axum::Form<Vec<(String, String)>>,
) -> Response {
    let form = form_map(pairs);
    let (profile, base, planned) = match prepare(&state, &auth, &form).await {
        Ok(ok) => ok,
        Err(response) => return response,
    };
    if planned.destructive
        && form.get("confirm").map(|c| c.trim()) != Some(planned.confirm.as_str())
    {
        return (
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"ok": false, "error": format!("Type {} to confirm", planned.confirm)})),
        )
            .into_response();
    }
    let target = profile.as_deref().unwrap_or(&base).to_string();
    let target = if profile.is_none() {
        state.effective_profile(None)
    } else {
        target
    };
    let description = planned.statements.join("; ");
    match state
        .run_ddl(&target, &planned.statements, planned.server_level)
        .await
    {
        Ok(()) => {
            record(&state, &auth, &base, "object action", audit::OK).await;
            Json(json!({"ok": true, "message": "Done", "sql": description})).into_response()
        }
        Err(message) => {
            record(&state, &auth, &base, "object action failed", audit::FAILED).await;
            (
                axum::http::StatusCode::BAD_GATEWAY,
                Json(json!({"ok": false, "error": message})),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn identifiers_are_quoted_and_injection_is_inert() {
        let plan = plan(
            Engine::PostgreSql,
            &form(&[
                ("scope", "table"),
                ("action", "rename"),
                ("schema", "public"),
                ("relation", "a\"; DROP TABLE x; --"),
                ("new_name", "b"),
            ]),
        )
        .unwrap();
        assert_eq!(
            plan.statements[0],
            "ALTER TABLE \"public\".\"a\"\"; DROP TABLE x; --\" RENAME TO \"b\""
        );
    }

    #[test]
    fn destructive_actions_demand_a_typed_name() {
        let drop = plan(
            Engine::Sqlite,
            &form(&[
                ("scope", "table"),
                ("action", "drop"),
                ("schema", "main"),
                ("relation", "tasks"),
            ]),
        )
        .unwrap();
        assert!(drop.destructive);
        assert_eq!(drop.confirm, "main.tasks");
        assert_eq!(drop.statements[0], "DROP TABLE \"main\".\"tasks\"");
        let rename = plan(
            Engine::Sqlite,
            &form(&[
                ("scope", "table"),
                ("action", "rename"),
                ("schema", "main"),
                ("relation", "t"),
                ("new_name", "u"),
            ]),
        )
        .unwrap();
        assert!(!rename.destructive);
    }

    #[test]
    fn unsafe_fragments_and_unsupported_actions_are_refused() {
        let bad_type = form(&[
            ("scope", "table"),
            ("action", "add-column"),
            ("schema", "main"),
            ("relation", "t"),
            ("col_name", "c"),
            ("col_type", "int; drop table t"),
        ]);
        assert!(plan(Engine::Sqlite, &bad_type).is_err());
        let comment = form(&[
            ("scope", "table"),
            ("action", "comment"),
            ("schema", "main"),
            ("relation", "t"),
            ("comment_text", "x"),
        ]);
        assert!(plan(Engine::Sqlite, &comment).is_err());
        assert!(plan(Engine::PostgreSql, &comment).is_ok());
    }
}
