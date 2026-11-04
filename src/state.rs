//! Shared server-owned state. No mutable global connection registry.

use std::sync::Arc;

use crate::{
    config::Config,
    db::{
        adapter::DatabaseAdapter,
        mysql::{MySqlManager, url::UrlError as MySqlUrlError},
        postgres::{PgManager, url::UrlError as PgUrlError},
        sqlite::{SqliteManager, SqliteUrlError},
        types::{ConnectionSnapshot, ConnectionStatus},
    },
};

#[derive(Debug)]
pub enum StateError {
    PostgreSql(PgUrlError),
    MySql(MySqlUrlError),
    Sqlite(SqliteUrlError),
    UnsupportedDatabase,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PostgreSql(error) => error.fmt(f),
            Self::MySql(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
            Self::UnsupportedDatabase => f.write_str("Unsupported database URL scheme"),
        }
    }
}

impl std::error::Error for StateError {}

#[derive(Clone)]
enum Runtime {
    PostgreSql(Arc<PgManager>),
    MySql(Arc<MySqlManager>),
    Sqlite(Arc<SqliteManager>),
}

#[derive(Clone)]
pub struct AppState {
    runtime: Option<Runtime>,
    catalog: Option<Arc<dyn DatabaseAdapter>>,
}

impl AppState {
    pub fn from_config(config: &Config) -> Result<Self, StateError> {
        let runtime = if let Some(secret) = config.database_url() {
            let url = secret.expose();
            if url.starts_with("sqlite:///") {
                let root = &config.sqlite_root;
                Some(Runtime::Sqlite(Arc::new(
                    SqliteManager::new(url, root.as_path(), &config.limits)
                        .map_err(StateError::Sqlite)?,
                )))
            } else if url.starts_with("sqlite:") {
                return Err(StateError::UnsupportedDatabase);
            } else if url.starts_with("mysql://") || url.starts_with("mariadb://") {
                Some(Runtime::MySql(Arc::new(
                    MySqlManager::new(url, &config.limits).map_err(StateError::MySql)?,
                )))
            } else {
                Some(Runtime::PostgreSql(Arc::new(
                    PgManager::new(url, &config.limits).map_err(StateError::PostgreSql)?,
                )))
            }
        } else {
            None
        };
        let catalog = runtime.as_ref().map(|manager| match manager {
            Runtime::PostgreSql(manager) => Arc::new(manager.catalog()) as Arc<dyn DatabaseAdapter>,
            Runtime::MySql(manager) => Arc::new(manager.catalog()) as Arc<dyn DatabaseAdapter>,
            Runtime::Sqlite(manager) => Arc::new(manager.catalog()) as Arc<dyn DatabaseAdapter>,
        });
        Ok(Self { runtime, catalog })
    }

    pub fn disconnected() -> Self {
        Self {
            runtime: None,
            catalog: None,
        }
    }

    /// Serve an already-built adapter (for isolated router tests with fakes).
    /// It reports no connection runtime, so it never opens a database.
    pub fn with_adapter(adapter: Arc<dyn DatabaseAdapter>) -> Self {
        Self {
            runtime: None,
            catalog: Some(adapter),
        }
    }

    pub async fn connection_snapshot(&self) -> ConnectionSnapshot {
        match &self.runtime {
            Some(Runtime::PostgreSql(manager)) => manager.snapshot().await,
            Some(Runtime::MySql(manager)) => manager.snapshot().await,
            Some(Runtime::Sqlite(manager)) => manager.snapshot().await,
            None => ConnectionSnapshot::disconnected(),
        }
    }

    pub async fn detected_engine(&self) -> Option<crate::db::types::Engine> {
        match &self.catalog {
            Some(catalog) => Some(catalog.detected_engine().await),
            None => None,
        }
    }

    /// The only catalog query target is the operator-configured shared pool.
    pub fn catalog(&self) -> Option<Arc<dyn DatabaseAdapter>> {
        self.catalog.clone()
    }

    /// Optional startup probe: errors change status, never crash the UI.
    pub fn start_probe(&self) {
        if let Some(manager) = &self.runtime {
            let manager = manager.clone();
            let _task = tokio::spawn(async move {
                match manager {
                    Runtime::PostgreSql(manager) => {
                        let _ = manager.snapshot().await;
                    }
                    Runtime::MySql(manager) => {
                        let _ = manager.snapshot().await;
                    }
                    Runtime::Sqlite(manager) => {
                        let _ = manager.snapshot().await;
                    }
                }
            });
        }
    }

    pub async fn close(&self) {
        match &self.runtime {
            Some(Runtime::PostgreSql(manager)) => manager.close().await,
            Some(Runtime::MySql(manager)) => manager.close().await,
            Some(Runtime::Sqlite(manager)) => manager.close().await,
            None => {}
        }
    }

    pub fn is_configured(&self) -> bool {
        self.runtime.is_some()
    }
}

impl ConnectionSnapshot {
    /// Static user-facing words, never operator credentials or raw driver text.
    pub fn label(self) -> &'static str {
        match self.status {
            ConnectionStatus::Disconnected => "Not configured",
            ConnectionStatus::Connecting => "Connecting",
            ConnectionStatus::Ready => "Connected",
            ConnectionStatus::Unavailable => "Unavailable",
            ConnectionStatus::AuthenticationFailed => "Authentication failed",
            ConnectionStatus::PermissionDenied => "Insufficient permissions",
        }
    }

    pub fn detail(self, detected_engine: Option<crate::db::types::Engine>) -> String {
        let engine = detected_engine.unwrap_or(self.engine);
        let engine = match engine {
            crate::db::types::Engine::PostgreSql => "PostgreSQL",
            crate::db::types::Engine::MySql => "MySQL",
            crate::db::types::Engine::MariaDb => "MariaDB",
            crate::db::types::Engine::Sqlite => "SQLite",
            crate::db::types::Engine::SqlServer => "SQL Server",
        };
        match self.status {
            ConnectionStatus::Disconnected => "Set CUADB_DATABASE_URL on the server.".into(),
            ConnectionStatus::Connecting => format!("Checking the configured {engine} connection."),
            ConnectionStatus::Ready => {
                format!("{engine} connection and read-only session verified.")
            }
            ConnectionStatus::Unavailable => {
                format!("Cannot reach the configured {engine} database.")
            }
            ConnectionStatus::AuthenticationFailed => {
                "Check the configured viewer credentials.".into()
            }
            ConnectionStatus::PermissionDenied => "The viewer role lacks required access.".into(),
        }
    }

    pub fn is_ready(self) -> bool {
        self.status == ConnectionStatus::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_url_remains_disconnected_without_network() {
        let config = Config::from_values(&[]).unwrap();
        let state = AppState::from_config(&config).unwrap();
        assert!(!state.is_configured());
        assert_eq!(
            state.connection_snapshot().await.status,
            ConnectionStatus::Disconnected
        );
        state.close().await;
    }

    #[test]
    fn malformed_connection_url_fails_at_startup_without_echoing_password() {
        let config = Config::from_values(&[(
            "CUADB_DATABASE_URL",
            "postgresql://viewer:secret@db/db?options=-c+bad",
        )])
        .unwrap();
        let outcome = AppState::from_config(&config).err().unwrap();
        assert!(!format!("{outcome:?}").contains("secret"));
    }
}
