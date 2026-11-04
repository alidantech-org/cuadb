//! SQLite persistence for users, sessions, grants and audit events.

use std::{path::Path, str::FromStr, time::Duration};

use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use super::{
    AuthError,
    audit::{AuditEvent, StoredEvent},
    now,
    rbac::Grant,
};

pub const DB_FILE: &str = "cuadb.db";
/// Audit rows younger than this cannot be deleted, even by SQL run against the file.
pub const AUDIT_RETENTION_DAYS: i64 = 90;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRow {
    pub id: i64,
    pub login: String,
    pub password_hash: String,
    pub role: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub user_id: i64,
    pub csrf: String,
    pub created_at: i64,
    pub last_seen: i64,
    pub expires_at: i64,
}

#[derive(Clone)]
pub struct AuthStore {
    /// Exposed so integration tests can age or inspect rows; never handed to routes.
    pub pool: SqlitePool,
}

fn storage<E>(_: E) -> AuthError {
    AuthError::Storage
}

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        login TEXT NOT NULL UNIQUE,
        password_hash TEXT NOT NULL,
        role TEXT NOT NULL CHECK (role IN ('admin','editor','viewer')),
        status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','disabled')),
        created_at INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sessions (
        id_hash BLOB PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(id),
        csrf TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        last_seen INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        revoked INTEGER NOT NULL DEFAULT 0
    )",
    "CREATE INDEX IF NOT EXISTS sessions_user ON sessions(user_id)",
    "CREATE TABLE IF NOT EXISTS grants (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL REFERENCES users(id),
        profile TEXT NOT NULL,
        schema_name TEXT NOT NULL DEFAULT '',
        relation_name TEXT NOT NULL DEFAULT '',
        actions INTEGER NOT NULL CHECK (actions BETWEEN 1 AND 15),
        UNIQUE (user_id, profile, schema_name, relation_name)
    )",
    "CREATE TABLE IF NOT EXISTS audit_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        ts INTEGER NOT NULL,
        actor_id INTEGER,
        actor TEXT NOT NULL,
        kind TEXT NOT NULL,
        profile TEXT NOT NULL DEFAULT '',
        resource TEXT NOT NULL DEFAULT '',
        outcome TEXT NOT NULL,
        detail TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS connections (
        id TEXT PRIMARY KEY,
        label TEXT NOT NULL,
        engine TEXT NOT NULL,
        url_enc BLOB NOT NULL,
        writable INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS audit_ts ON audit_events(ts)",
    "CREATE TRIGGER IF NOT EXISTS audit_no_update BEFORE UPDATE ON audit_events
        BEGIN SELECT RAISE(ABORT, 'audit events are append-only'); END",
    "CREATE TRIGGER IF NOT EXISTS audit_retained BEFORE DELETE ON audit_events
        WHEN OLD.ts > CAST(strftime('%s','now') AS INTEGER) - 90 * 86400
        BEGIN SELECT RAISE(ABORT, 'audit events are retained'); END",
];

impl AuthStore {
    /// Open (and create if `create`) `dir/cuadb.db` with owner-only permissions.
    pub async fn open(dir: &Path, create: bool) -> Result<Self, AuthError> {
        let file = dir.join(DB_FILE);
        if !create && !file.is_file() {
            return Err(AuthError::Storage);
        }
        if create {
            std::fs::create_dir_all(dir).map_err(storage)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                    .map_err(storage)?;
            }
        }
        let options = SqliteConnectOptions::new()
            .filename(&file)
            .create_if_missing(create)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(storage)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
        }
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// An isolated in-memory store (one connection) for tests.
    pub async fn open_memory() -> Result<Self, AuthError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(storage)?
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(storage)?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<(), AuthError> {
        for statement in SCHEMA {
            sqlx::query(statement)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        }
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn meta(&self, key: &str) -> Result<Option<String>, AuthError> {
        sqlx::query_scalar("SELECT value FROM meta WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }

    pub async fn user_count(&self) -> Result<i64, AuthError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    pub async fn active_admin_count(&self) -> Result<i64, AuthError> {
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin' AND status = 'active'")
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    /// Create the first administrator and mark bootstrap done, atomically.
    pub async fn bootstrap_admin(&self, login: &str, hash: &str) -> Result<i64, AuthError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let marker: Option<String> =
            sqlx::query_scalar("SELECT value FROM meta WHERE key = 'bootstrapped_at'")
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        if users > 0 || marker.is_some() {
            return Err(AuthError::BootstrapRefused(
                "setup already happened or state is ambiguous",
            ));
        }
        let id = sqlx::query(
            "INSERT INTO users (login, password_hash, role, created_at) VALUES (?, ?, 'admin', ?)",
        )
        .bind(login)
        .bind(hash)
        .bind(now())
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .last_insert_rowid();
        sqlx::query("INSERT INTO meta (key, value) VALUES ('bootstrapped_at', ?)")
            .bind(now().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(id)
    }

    pub async fn create_user(&self, login: &str, hash: &str, role: &str) -> Result<i64, AuthError> {
        let result = sqlx::query(
            "INSERT INTO users (login, password_hash, role, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(login)
        .bind(hash)
        .bind(role)
        .bind(now())
        .execute(&self.pool)
        .await;
        match result {
            Ok(done) => Ok(done.last_insert_rowid()),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(AuthError::Exists),
            Err(_) => Err(AuthError::Storage),
        }
    }

    fn user_from(row: &sqlx::sqlite::SqliteRow) -> UserRow {
        UserRow {
            id: row.get("id"),
            login: row.get("login"),
            password_hash: row.get("password_hash"),
            role: row.get("role"),
            active: row.get::<String, _>("status") == "active",
        }
    }

    pub async fn user_by_login(&self, login: &str) -> Result<Option<UserRow>, AuthError> {
        let row =
            sqlx::query("SELECT id, login, password_hash, role, status FROM users WHERE login = ?")
                .bind(login)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        Ok(row.as_ref().map(Self::user_from))
    }

    pub async fn user_by_id(&self, id: i64) -> Result<Option<UserRow>, AuthError> {
        let row =
            sqlx::query("SELECT id, login, password_hash, role, status FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        Ok(row.as_ref().map(Self::user_from))
    }

    pub async fn list_users(&self) -> Result<Vec<UserRow>, AuthError> {
        let rows = sqlx::query(
            "SELECT id, login, password_hash, role, status FROM users ORDER BY login LIMIT 500",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows.iter().map(Self::user_from).collect())
    }

    pub async fn set_user_active(&self, id: i64, active: bool) -> Result<(), AuthError> {
        let done = sqlx::query("UPDATE users SET status = ? WHERE id = ?")
            .bind(if active { "active" } else { "disabled" })
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        if done.rows_affected() == 0 {
            Err(AuthError::NotFound)
        } else {
            Ok(())
        }
    }

    pub async fn set_password(&self, id: i64, hash: &str) -> Result<(), AuthError> {
        let done = sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(hash)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        if done.rows_affected() == 0 {
            Err(AuthError::NotFound)
        } else {
            Ok(())
        }
    }

    // ---- sessions

    pub async fn create_session(
        &self,
        id_hash: &[u8],
        user_id: i64,
        csrf: &str,
        expires_at: i64,
    ) -> Result<(), AuthError> {
        let t = now();
        sqlx::query("INSERT INTO sessions (id_hash, user_id, csrf, created_at, last_seen, expires_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id_hash)
            .bind(user_id)
            .bind(csrf)
            .bind(t)
            .bind(t)
            .bind(expires_at)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    pub async fn session(&self, id_hash: &[u8]) -> Result<Option<SessionRow>, AuthError> {
        let row = sqlx::query("SELECT user_id, csrf, created_at, last_seen, expires_at FROM sessions WHERE id_hash = ? AND revoked = 0")
            .bind(id_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        Ok(row.map(|r| SessionRow {
            user_id: r.get("user_id"),
            csrf: r.get("csrf"),
            created_at: r.get("created_at"),
            last_seen: r.get("last_seen"),
            expires_at: r.get("expires_at"),
        }))
    }

    pub async fn touch_session(&self, id_hash: &[u8]) -> Result<(), AuthError> {
        sqlx::query("UPDATE sessions SET last_seen = ? WHERE id_hash = ?")
            .bind(now())
            .bind(id_hash)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    pub async fn revoke_session(&self, id_hash: &[u8]) -> Result<(), AuthError> {
        sqlx::query("UPDATE sessions SET revoked = 1 WHERE id_hash = ?")
            .bind(id_hash)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    pub async fn revoke_user_sessions(&self, user_id: i64) -> Result<u64, AuthError> {
        sqlx::query("UPDATE sessions SET revoked = 1 WHERE user_id = ? AND revoked = 0")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|done| done.rows_affected())
            .map_err(storage)
    }

    /// Remove expired and revoked sessions (bounded housekeeping).
    pub async fn purge_sessions(&self) -> Result<u64, AuthError> {
        sqlx::query("DELETE FROM sessions WHERE revoked = 1 OR expires_at < ?")
            .bind(now() - 86_400)
            .execute(&self.pool)
            .await
            .map(|done| done.rows_affected())
            .map_err(storage)
    }

    // ---- grants

    pub async fn add_grant(
        &self,
        user_id: i64,
        profile: &str,
        schema: &str,
        relation: &str,
        actions: u8,
    ) -> Result<i64, AuthError> {
        if self.user_by_id(user_id).await?.is_none() {
            return Err(AuthError::NotFound);
        }
        sqlx::query(
            "INSERT INTO grants (user_id, profile, schema_name, relation_name, actions) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(user_id, profile, schema_name, relation_name) DO UPDATE SET actions = excluded.actions",
        )
        .bind(user_id)
        .bind(profile)
        .bind(schema)
        .bind(relation)
        .bind(i64::from(actions))
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        sqlx::query_scalar("SELECT id FROM grants WHERE user_id = ? AND profile = ? AND schema_name = ? AND relation_name = ?")
            .bind(user_id)
            .bind(profile)
            .bind(schema)
            .bind(relation)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)
    }

    pub async fn remove_grant(&self, id: i64) -> Result<(), AuthError> {
        let done = sqlx::query("DELETE FROM grants WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        if done.rows_affected() == 0 {
            Err(AuthError::NotFound)
        } else {
            Ok(())
        }
    }

    pub async fn grants_for_user(&self, user_id: i64) -> Result<Vec<Grant>, AuthError> {
        let rows = sqlx::query("SELECT id, profile, schema_name, relation_name, actions FROM grants WHERE user_id = ? ORDER BY id LIMIT 1000")
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| Grant {
                id: r.get("id"),
                profile: r.get("profile"),
                schema: r.get("schema_name"),
                relation: r.get("relation_name"),
                actions: r.get::<i64, _>("actions") as u8,
            })
            .collect())
    }

    // ---- audit

    pub async fn audit(&self, event: &AuditEvent) -> Result<(), AuthError> {
        sqlx::query("INSERT INTO audit_events (ts, actor_id, actor, kind, profile, resource, outcome, detail) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(now())
            .bind(event.actor_id)
            .bind(&event.actor)
            .bind(event.kind)
            .bind(event.profile.as_deref().unwrap_or(""))
            .bind(event.resource.as_deref().unwrap_or(""))
            .bind(event.outcome)
            .bind(event.detail.as_deref().unwrap_or(""))
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage)
    }

    /// Newest first, at most `limit` (<= 200) rows with `id < before`.
    pub async fn audit_events(
        &self,
        limit: i64,
        before: Option<i64>,
    ) -> Result<Vec<StoredEvent>, AuthError> {
        let rows = sqlx::query(
            "SELECT id, ts, actor, kind, profile, resource, outcome, detail FROM audit_events
             WHERE id < ? ORDER BY id DESC LIMIT ?",
        )
        .bind(before.unwrap_or(i64::MAX))
        .bind(limit.clamp(1, 200))
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| StoredEvent {
                id: r.get("id"),
                ts: r.get("ts"),
                actor: r.get("actor"),
                kind: r.get("kind"),
                profile: r.get("profile"),
                resource: r.get("resource"),
                outcome: r.get("outcome"),
                detail: r.get("detail"),
            })
            .collect())
    }

    /// The only deletion path: rows older than the retention window.
    pub async fn prune_audit(&self) -> Result<u64, AuthError> {
        sqlx::query("DELETE FROM audit_events WHERE ts < ?")
            .bind(now() - AUDIT_RETENTION_DAYS * 86_400 - 1)
            .execute(&self.pool)
            .await
            .map(|done| done.rows_affected())
            .map_err(storage)
    }
}

/// A saved connection. `url_enc` is AES-GCM ciphertext; this type never holds a plain URL.
#[derive(Debug, Clone)]
pub struct ConnectionRow {
    pub id: String,
    pub label: String,
    pub engine: String,
    pub url_enc: Vec<u8>,
    pub writable: bool,
}

impl AuthStore {
    pub async fn list_connections(&self) -> Result<Vec<ConnectionRow>, AuthError> {
        use sqlx::Row;
        let rows = sqlx::query(
            "SELECT id, label, engine, url_enc, writable FROM connections ORDER BY created_at, id",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| ConnectionRow {
                id: r.get("id"),
                label: r.get("label"),
                engine: r.get("engine"),
                url_enc: r.get("url_enc"),
                writable: r.get::<i64, _>("writable") != 0,
            })
            .collect())
    }

    pub async fn connection(&self, id: &str) -> Result<Option<ConnectionRow>, AuthError> {
        Ok(self
            .list_connections()
            .await?
            .into_iter()
            .find(|row| row.id == id))
    }

    pub async fn save_connection(&self, row: &ConnectionRow) -> Result<(), AuthError> {
        sqlx::query(
            "INSERT INTO connections (id, label, engine, url_enc, writable, created_at)
             VALUES (?, ?, ?, ?, ?, CAST(strftime('%s','now') AS INTEGER))
             ON CONFLICT(id) DO UPDATE SET label = excluded.label, engine = excluded.engine,
               url_enc = excluded.url_enc, writable = excluded.writable",
        )
        .bind(&row.id)
        .bind(&row.label)
        .bind(&row.engine)
        .bind(&row.url_enc)
        .bind(i64::from(row.writable))
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(())
    }

    /// Removes the connection and every grant that named it.
    pub async fn delete_connection(&self, id: &str) -> Result<bool, AuthError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("DELETE FROM grants WHERE profile = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let done = sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?
            .rows_affected();
        tx.commit().await.map_err(storage)?;
        Ok(done > 0)
    }
}

/// Filters for the audit viewer. Every field is optional; `since`/`until` are Unix seconds.
#[derive(Debug, Default, Clone)]
pub struct AuditFilter {
    pub actor: Option<String>,
    pub kind: Option<String>,
    pub outcome: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
}

impl AuthStore {
    pub async fn set_user_role(&self, id: i64, role: &str) -> Result<(), AuthError> {
        let done = sqlx::query("UPDATE users SET role = ? WHERE id = ?")
            .bind(role)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        if done.rows_affected() == 0 {
            Err(AuthError::NotFound)
        } else {
            Ok(())
        }
    }

    /// Remove a user with their sessions and grants. Audit rows stay (they hold only the
    /// login text), so the history of what that person did survives.
    pub async fn delete_user(&self, id: i64) -> Result<(), AuthError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("DELETE FROM grants WHERE user_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let done = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        if done.rows_affected() == 0 {
            Err(AuthError::NotFound)
        } else {
            Ok(())
        }
    }

    pub async fn audit_events_filtered(
        &self,
        filter: &AuditFilter,
        limit: i64,
        before: Option<i64>,
    ) -> Result<Vec<StoredEvent>, AuthError> {
        let rows = sqlx::query(
            "SELECT id, ts, actor, kind, profile, resource, outcome, detail FROM audit_events
             WHERE id < ?1
               AND (?2 IS NULL OR actor = ?2)
               AND (?3 IS NULL OR kind = ?3)
               AND (?4 IS NULL OR outcome = ?4)
               AND (?5 IS NULL OR ts >= ?5)
               AND (?6 IS NULL OR ts < ?6)
             ORDER BY id DESC LIMIT ?7",
        )
        .bind(before.unwrap_or(i64::MAX))
        .bind(filter.actor.as_deref())
        .bind(filter.kind.as_deref())
        .bind(filter.outcome.as_deref())
        .bind(filter.since)
        .bind(filter.until)
        .bind(limit.clamp(1, 200))
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        Ok(rows
            .iter()
            .map(|r| StoredEvent {
                id: r.get("id"),
                ts: r.get("ts"),
                actor: r.get("actor"),
                kind: r.get("kind"),
                profile: r.get("profile"),
                resource: r.get("resource"),
                outcome: r.get("outcome"),
                detail: r.get("detail"),
            })
            .collect())
    }
}
