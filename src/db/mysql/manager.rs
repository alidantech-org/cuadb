//! Bounded shared MySQL/MariaDB pool with redacted, cached health probes.
use std::{sync::Arc, time::Duration};

use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use tokio::{sync::RwLock, time::timeout};

use crate::{
    config::Limits,
    db::types::{ConnectionSnapshot, ConnectionStatus, DbError, DbErrorKind, Engine},
};

use super::url::{self, UrlError};

#[derive(Clone)]
pub struct MySqlManager {
    pool: MySqlPool,
    limits: Limits,
    state: Arc<RwLock<SnapshotState>>,
    engine: Arc<RwLock<Engine>>,
}
struct SnapshotState {
    snapshot: ConnectionSnapshot,
    checked: Option<std::time::Instant>,
}

impl MySqlManager {
    pub fn new(raw: &str, limits: &Limits) -> Result<Self, UrlError> {
        let initial_flavor = if raw.starts_with("mariadb://") {
            Engine::MariaDb
        } else {
            Engine::MySql
        };
        let options = url::parse(raw)?.into_options();
        let milliseconds = limits.query_timeout.as_millis() as u64;
        let pool = MySqlPoolOptions::new()
            .max_connections(limits.max_pool_connections)
            .min_connections(0)
            .acquire_timeout(limits.connection_timeout)
            .idle_timeout(Duration::from_secs(120))
            .max_lifetime(Duration::from_secs(1800))
            .after_connect(move |connection, _| {
                let milliseconds = milliseconds;
                Box::pin(async move {
                    // Server-side privileges remain mandatory; this adds a
                    // read-only default for pooled sessions as defense in depth.
                    sqlx::query("SET SESSION TRANSACTION READ ONLY")
                        .execute(&mut *connection)
                        .await?;
                    // Ask the server to abort long SELECTs instead of only abandoning them
                    // client-side. MySQL: max_execution_time (ms); MariaDB: max_statement_time
                    // (seconds). Each engine rejects the other's variable, which is expected.
                    let _ =
                        sqlx::query(&format!("SET SESSION max_execution_time = {milliseconds}"))
                            .execute(&mut *connection)
                            .await;
                    let _ = sqlx::query(&format!(
                        "SET SESSION max_statement_time = {}",
                        milliseconds as f64 / 1000.0
                    ))
                    .execute(&mut *connection)
                    .await;
                    Ok(())
                })
            })
            .connect_lazy_with(options);
        Ok(Self {
            pool,
            limits: limits.clone(),
            state: Arc::new(RwLock::new(SnapshotState {
                snapshot: ConnectionSnapshot {
                    engine: initial_flavor,
                    status: ConnectionStatus::Connecting,
                    error_code: None,
                },
                checked: None,
            })),
            engine: Arc::new(RwLock::new(initial_flavor)),
        })
    }

    pub async fn snapshot(&self) -> ConnectionSnapshot {
        const RECHECK: Duration = Duration::from_secs(8);
        {
            let cached = self.state.read().await;
            if cached.checked.is_some_and(|at| at.elapsed() < RECHECK) {
                return cached.snapshot;
            }
        }
        let probed = match timeout(
            self.limits.connection_timeout + self.limits.query_timeout,
            sqlx::query_scalar::<_, String>("SELECT VERSION()").fetch_one(&self.pool),
        )
        .await
        {
            Ok(Ok(version)) => Ok(version),
            Ok(Err(error)) => Err(classify(error)),
            Err(_) => Err(DbError::new(DbErrorKind::TimedOut)),
        };
        let snapshot = match probed {
            Ok(version) => ConnectionSnapshot {
                engine: if version.to_ascii_lowercase().contains("mariadb") {
                    Engine::MariaDb
                } else {
                    Engine::MySql
                },
                status: ConnectionStatus::Ready,
                error_code: None,
            },
            Err(error) => {
                eprintln!("level=warn event=database_probe code={}", error.code());
                ConnectionSnapshot {
                    engine: *self.engine.read().await,
                    status: match error.kind {
                        DbErrorKind::PermissionDenied => ConnectionStatus::PermissionDenied,
                        DbErrorKind::AuthenticationFailed => ConnectionStatus::AuthenticationFailed,
                        _ => ConnectionStatus::Unavailable,
                    },
                    error_code: Some(error.code()),
                }
            }
        };
        *self.engine.write().await = snapshot.engine;
        let mut state = self.state.write().await;
        state.snapshot = snapshot;
        state.checked = Some(std::time::Instant::now());
        snapshot
    }

    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    pub fn catalog(&self) -> super::catalog::MySqlCatalog {
        super::catalog::MySqlCatalog::new(
            self.pool.clone(),
            self.limits.query_timeout,
            self.limits.max_page_size,
        )
        .with_engine_state(Arc::clone(&self.engine))
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

pub(crate) fn classify(error: sqlx::Error) -> DbError {
    // `database.code()` is the SQLSTATE ("42000", "70100"), which is shared by many
    // different errors; the driver's numeric error code is the precise discriminator.
    let database = error.as_database_error();
    let number = database
        .and_then(|database| database.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
        .map(|mysql| mysql.number());
    let sqlstate = database.and_then(|database| database.code());
    let kind = match (number, sqlstate.as_deref()) {
        (Some(1044 | 1045), _) | (_, Some("28000")) => DbErrorKind::AuthenticationFailed,
        (Some(1142 | 1143 | 1227 | 1370), _) => DbErrorKind::PermissionDenied,
        // 1205 lock wait, 1317 interrupted, 1969 MariaDB max_statement_time,
        // 3024 MySQL max_execution_time.
        (Some(1205 | 1317 | 1969 | 3024), _) | (_, Some("70100")) => DbErrorKind::TimedOut,
        _ => DbErrorKind::ConnectionUnavailable,
    };
    DbError::new(kind)
}

#[cfg(test)]
mod classify_tests {
    use super::*;

    #[test]
    fn non_database_errors_are_unavailable_and_secret_free() {
        assert_eq!(
            classify(sqlx::Error::PoolTimedOut).kind,
            DbErrorKind::ConnectionUnavailable
        );
    }
}
