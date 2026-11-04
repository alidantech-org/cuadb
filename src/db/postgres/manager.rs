//! Bounded PostgreSQL connection lifecycle and read-only session bootstrap.
//!
//! No browser request ever supplies a URL or SQL statement. All pool probes
//! use constant introspection SQL; later catalog adapters own their own queries.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{
    sync::{Mutex, RwLock},
    time::timeout,
};

use crate::{
    config::Limits,
    db::types::{ConnectionSnapshot, ConnectionStatus, DbError, DbErrorKind, Engine},
};

use super::url::{self, UrlError};

/// One pool shared by all HTTP handlers, with a cached and redacted status.
#[derive(Clone)]
pub struct PgManager {
    pool: PgPool,
    limits: Limits,
    cache: Arc<RwLock<Cache>>,
    probe_gate: Arc<Mutex<()>>,
}

struct Cache {
    snapshot: ConnectionSnapshot,
    checked_at: Option<Instant>,
}

impl PgManager {
    /// Construct a pool without connecting. This keeps disconnected or failed
    /// databases from preventing the HTTP shell from starting.
    pub fn new(raw: &str, limits: &Limits) -> Result<Self, UrlError> {
        let options = url::parse(raw)?.into_options();
        let milliseconds = limits.query_timeout.as_millis().to_string();

        let pool = PgPoolOptions::new()
            .max_connections(limits.max_pool_connections)
            .min_connections(0)
            .acquire_timeout(limits.connection_timeout)
            .idle_timeout(Duration::from_secs(120))
            .max_lifetime(Duration::from_secs(1800))
            .after_connect(move |connection, _| {
                let milliseconds = milliseconds.clone();
                Box::pin(async move {
                    // This is a supplementary guard, not a substitute for a
                    // restricted SELECT-only database role.
                    sqlx::query("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SELECT pg_catalog.set_config('statement_timeout', $1, false)")
                        .bind(milliseconds.as_str())
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("SELECT pg_catalog.set_config('lock_timeout', $1, false)")
                        .bind(milliseconds.as_str())
                        .execute(&mut *connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_lazy_with(options);

        Ok(Self {
            pool,
            limits: limits.clone(),
            cache: Arc::new(RwLock::new(Cache {
                snapshot: ConnectionSnapshot {
                    engine: Engine::PostgreSql,
                    status: ConnectionStatus::Connecting,
                    error_code: None,
                },
                checked_at: None,
            })),
            probe_gate: Arc::new(Mutex::new(())),
        })
    }

    /// Refresh at most once per short interval; concurrent callers share the
    /// same probe rather than opening one connection per HTMX request.
    pub async fn snapshot(&self) -> ConnectionSnapshot {
        const RECHECK_AFTER: Duration = Duration::from_secs(8);
        {
            let state = self.cache.read().await;
            if state
                .checked_at
                .is_some_and(|at| at.elapsed() < RECHECK_AFTER)
            {
                return state.snapshot;
            }
        }

        let _gate = self.probe_gate.lock().await;
        {
            let state = self.cache.read().await;
            if state
                .checked_at
                .is_some_and(|at| at.elapsed() < RECHECK_AFTER)
            {
                return state.snapshot;
            }
        }

        let outcome = self.probe().await;
        let snapshot = match outcome {
            Ok(()) => ConnectionSnapshot {
                engine: Engine::PostgreSql,
                status: ConnectionStatus::Ready,
                error_code: None,
            },
            Err(error) => {
                let status = match error.kind {
                    DbErrorKind::PermissionDenied => ConnectionStatus::PermissionDenied,
                    DbErrorKind::AuthenticationFailed => ConnectionStatus::AuthenticationFailed,
                    _ => ConnectionStatus::Unavailable,
                };
                eprintln!("level=warn event=database_probe code={}", error.code());
                ConnectionSnapshot {
                    engine: Engine::PostgreSql,
                    status,
                    error_code: Some(error.code()),
                }
            }
        };
        let mut state = self.cache.write().await;
        state.snapshot = snapshot;
        state.checked_at = Some(Instant::now());
        snapshot
    }

    /// One constant, bounded read-only probe. Never run arbitrary client SQL.
    async fn probe(&self) -> Result<(), DbError> {
        let budget = self.limits.connection_timeout + self.limits.query_timeout;
        let result = timeout(budget, async {
            let mut connection = self.pool.acquire().await.map_err(classify)?;
            let mode: String =
                sqlx::query_scalar("SELECT pg_catalog.current_setting('transaction_read_only')")
                    .fetch_one(&mut *connection)
                    .await
                    .map_err(classify)?;

            if mode != "on" {
                return Err(DbError::new(DbErrorKind::PermissionDenied));
            }

            // Distinguish a valid login with no usable application schemas.
            // This does NOT assert SELECT privileges on every table.
            let can_inspect: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_namespace n \
                 WHERE n.nspname NOT LIKE 'pg_%' \
                 AND n.nspname <> 'information_schema' \
                 AND pg_catalog.has_schema_privilege(n.oid, 'USAGE'))",
            )
            .fetch_one(&mut *connection)
            .await
            .map_err(classify)?;

            if !can_inspect {
                return Err(DbError::new(DbErrorKind::PermissionDenied));
            }
            Ok(())
        })
        .await;

        result.map_err(|_| DbError::new(DbErrorKind::TimedOut))?
    }

    /// Called after HTTP shutdown; no new SQL work should be started.
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// A small clone of the single shared pool for permission-scoped metadata.
    pub fn catalog(&self) -> super::catalog::PgCatalog {
        super::catalog::PgCatalog {
            pool: self.pool.clone(),
            deadline: self.limits.query_timeout,
        }
    }

    /// Report whether pooled sessions enforce both current and default
    /// transaction read-only settings. Only constant SELECT queries are used.
    pub async fn read_only_session_verified(&self) -> Result<bool, DbError> {
        let mut conn = self.pool.acquire().await.map_err(classify)?;
        let current: String =
            sqlx::query_scalar("SELECT pg_catalog.current_setting('transaction_read_only')")
                .fetch_one(&mut *conn)
                .await
                .map_err(classify)?;
        let default: String = sqlx::query_scalar(
            "SELECT pg_catalog.current_setting('default_transaction_read_only')",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(classify)?;
        Ok(current == "on" && default == "on")
    }
}

pub(super) fn classify(error: sqlx::Error) -> DbError {
    let kind = match &error {
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some("28P01" | "28000") => DbErrorKind::AuthenticationFailed,
            Some("42501") => DbErrorKind::PermissionDenied,
            Some("57014") => DbErrorKind::TimedOut,
            _ => DbErrorKind::ConnectionUnavailable,
        },
        sqlx::Error::PoolTimedOut => DbErrorKind::TimedOut,
        _ => DbErrorKind::ConnectionUnavailable,
    };
    // The driver error may contain credentials; never store or display it.
    DbError::new(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pool_constructor_is_lazy_and_never_opens_a_socket() {
        let config = crate::config::Config::from_values(&[]).unwrap();
        let manager = PgManager::new(
            "postgresql://viewer:secret@127.0.0.1:1/no_db?sslmode=disable",
            &config.limits,
        )
        .unwrap();
        assert_eq!(manager.pool.size(), 0);
    }

    #[test]
    fn postgres_error_categories_are_redacted() {
        let error = classify(sqlx::Error::PoolTimedOut);
        assert_eq!(error.kind, DbErrorKind::TimedOut);
        assert!(!format!("{error:?}").contains("secret"));
    }
}
