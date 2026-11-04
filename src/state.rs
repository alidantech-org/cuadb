//! Shared server-owned state. No mutable global connection registry.

use std::sync::Arc;

use crate::{
    config::{Config, Limits},
    db::{
        adapter::DatabaseAdapter,
        mysql::{MySqlManager, url::UrlError as MySqlUrlError},
        postgres::{PgManager, url::UrlError as PgUrlError},
        sqlite::{SqliteManager, SqliteUrlError},
        types::{ConnectionSnapshot, ConnectionStatus},
    },
    profiles::ProfileSet,
};

#[derive(Debug)]
pub enum StateError {
    PostgreSql(PgUrlError),
    MySql(MySqlUrlError),
    Sqlite(SqliteUrlError),
    UnsupportedDatabase,
    /// The profiles file could not be read or its contents are invalid (secret-free).
    Profiles(String),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PostgreSql(error) => error.fmt(f),
            Self::MySql(error) => error.fmt(f),
            Self::Sqlite(error) => error.fmt(f),
            Self::UnsupportedDatabase => f.write_str("Unsupported database URL scheme"),
            Self::Profiles(message) => f.write_str(message),
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

impl Runtime {
    fn catalog(&self) -> Arc<dyn DatabaseAdapter> {
        match self {
            Self::PostgreSql(manager) => Arc::new(manager.catalog()),
            Self::MySql(manager) => Arc::new(manager.catalog()),
            Self::Sqlite(manager) => Arc::new(manager.catalog()),
        }
    }

    async fn snapshot(&self) -> ConnectionSnapshot {
        match self {
            Self::PostgreSql(manager) => manager.snapshot().await,
            Self::MySql(manager) => manager.snapshot().await,
            Self::Sqlite(manager) => manager.snapshot().await,
        }
    }

    async fn close(&self) {
        match self {
            Self::PostgreSql(manager) => manager.close().await,
            Self::MySql(manager) => manager.close().await,
            Self::Sqlite(manager) => manager.close().await,
        }
    }

    fn family(&self) -> &'static str {
        match self {
            Self::PostgreSql(_) => "PostgreSQL",
            Self::MySql(_) => "MySQL / MariaDB",
            Self::Sqlite(_) => "SQLite",
        }
    }
}

struct Entry {
    id: String,
    label: String,
    runtime: Option<Runtime>,
    catalog: Arc<dyn DatabaseAdapter>,
}

/// Non-secret facts about a configured profile, safe for templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    pub id: String,
    pub label: String,
    pub engine: &'static str,
    pub is_default: bool,
}

/// Result of resolving the `profile` request parameter.
pub enum Lookup {
    Found(Arc<dyn DatabaseAdapter>),
    /// No database is configured at all.
    NotConfigured,
    /// The browser named a profile that is not configured (or was removed).
    Unknown,
}

/// Immutable registry of trusted profiles. Pools are created lazily by each
/// manager and closed when idle; the browser selects only by profile id.
#[derive(Clone)]
pub struct AppState {
    entries: Arc<Vec<Entry>>,
    default: usize,
}

fn build_runtime(
    url: &str,
    root: &std::path::Path,
    limits: &Limits,
) -> Result<Runtime, StateError> {
    if url.starts_with("sqlite:///") {
        Ok(Runtime::Sqlite(Arc::new(
            SqliteManager::new(url, root, limits).map_err(StateError::Sqlite)?,
        )))
    } else if url.starts_with("sqlite:") {
        Err(StateError::UnsupportedDatabase)
    } else if url.starts_with("mysql://") || url.starts_with("mariadb://") {
        Ok(Runtime::MySql(Arc::new(
            MySqlManager::new(url, limits).map_err(StateError::MySql)?,
        )))
    } else {
        Ok(Runtime::PostgreSql(Arc::new(
            PgManager::new(url, limits).map_err(StateError::PostgreSql)?,
        )))
    }
}

impl AppState {
    pub fn from_config(config: &Config) -> Result<Self, StateError> {
        let set = if let Some(path) = config.profiles_file() {
            let text = std::fs::read_to_string(path).map_err(|_| {
                StateError::Profiles("CUADB_PROFILES_FILE cannot be read as UTF-8 text".into())
            })?;
            Some(ProfileSet::from_json(&text).map_err(|e| StateError::Profiles(e.to_string()))?)
        } else {
            config.database_url().cloned().map(ProfileSet::single)
        };
        let Some(set) = set else {
            return Ok(Self::disconnected());
        };
        // Divide the pool budget so many profiles cannot multiply open connections.
        let mut limits = config.limits.clone();
        limits.max_pool_connections =
            (limits.max_pool_connections / set.profiles.len() as u32).max(1);
        let mut entries = Vec::new();
        for profile in &set.profiles {
            let runtime = build_runtime(profile.url.expose(), &config.sqlite_root, &limits)?;
            entries.push(Entry {
                id: profile.id.clone(),
                label: profile.label.clone(),
                catalog: runtime.catalog(),
                runtime: Some(runtime),
            });
        }
        Ok(Self {
            entries: Arc::new(entries),
            default: set.default,
        })
    }

    pub fn disconnected() -> Self {
        Self {
            entries: Arc::new(Vec::new()),
            default: 0,
        }
    }

    /// Serve an already-built adapter (for isolated router tests with fakes).
    /// It reports no connection runtime, so it never opens a database.
    pub fn with_adapter(adapter: Arc<dyn DatabaseAdapter>) -> Self {
        Self {
            entries: Arc::new(vec![Entry {
                id: crate::profiles::DEFAULT_PROFILE_ID.into(),
                label: "Default connection".into(),
                runtime: None,
                catalog: adapter,
            }]),
            default: 0,
        }
    }

    /// Several fake adapters keyed by profile id, for isolated tests.
    pub fn with_adapters(adapters: Vec<(&str, &str, Arc<dyn DatabaseAdapter>)>) -> Self {
        Self {
            entries: Arc::new(
                adapters
                    .into_iter()
                    .map(|(id, label, catalog)| Entry {
                        id: id.into(),
                        label: label.into(),
                        runtime: None,
                        catalog,
                    })
                    .collect(),
            ),
            default: 0,
        }
    }

    fn default_entry(&self) -> Option<&Entry> {
        self.entries.get(self.default)
    }

    fn entry(&self, profile: Option<&str>) -> Option<&Entry> {
        match profile {
            None => self.default_entry(),
            Some(id) => self.entries.iter().find(|entry| entry.id == id),
        }
    }

    pub async fn connection_snapshot(&self) -> ConnectionSnapshot {
        self.profile_snapshot(None)
            .await
            .unwrap_or_else(ConnectionSnapshot::disconnected)
    }

    /// `None` when the profile is unknown. Profiles without a runtime (test
    /// adapters) report disconnected.
    pub async fn profile_snapshot(&self, profile: Option<&str>) -> Option<ConnectionSnapshot> {
        let entry = self.entry(profile)?;
        Some(match &entry.runtime {
            Some(runtime) => runtime.snapshot().await,
            None => ConnectionSnapshot::disconnected(),
        })
    }

    pub async fn detected_engine(&self) -> Option<crate::db::types::Engine> {
        match self.default_entry() {
            Some(entry) => Some(entry.catalog.detected_engine().await),
            None => None,
        }
    }

    pub async fn profile_engine(&self, profile: Option<&str>) -> Option<crate::db::types::Engine> {
        let entry = self.entry(profile)?;
        Some(entry.catalog.detected_engine().await)
    }

    /// Catalog of the default profile.
    pub fn catalog(&self) -> Option<Arc<dyn DatabaseAdapter>> {
        self.default_entry().map(|entry| entry.catalog.clone())
    }

    /// Resolve the request's `profile` parameter (absent means the default).
    pub fn catalog_for(&self, profile: Option<&str>) -> Lookup {
        match (profile, self.entry(profile)) {
            (_, Some(entry)) => Lookup::Found(entry.catalog.clone()),
            (None, None) => Lookup::NotConfigured,
            (Some(_), None) => Lookup::Unknown,
        }
    }

    /// Configured profiles in file order. Never includes URLs.
    pub fn profiles(&self) -> Vec<ProfileInfo> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| ProfileInfo {
                id: entry.id.clone(),
                label: entry.label.clone(),
                engine: entry
                    .runtime
                    .as_ref()
                    .map_or("Test adapter", Runtime::family),
                is_default: index == self.default,
            })
            .collect()
    }

    pub fn default_profile_id(&self) -> Option<&str> {
        self.default_entry().map(|entry| entry.id.as_str())
    }

    /// Optional startup probe of the default profile: errors change status, never crash.
    pub fn start_probe(&self) {
        if let Some(runtime) = self.default_entry().and_then(|e| e.runtime.clone()) {
            let _task = tokio::spawn(async move {
                let _ = runtime.snapshot().await;
            });
        }
    }

    pub async fn close(&self) {
        for entry in self.entries.iter() {
            if let Some(runtime) = &entry.runtime {
                runtime.close().await;
            }
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.entries.is_empty()
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

/// Validate the optional `profile` request parameter (syntax only).
pub fn checked_profile(value: Option<String>) -> Result<Option<String>, crate::error::AppError> {
    match value {
        None => Ok(None),
        Some(id) if crate::profiles::valid_id(&id) => Ok(Some(id)),
        Some(_) => Err(crate::error::AppError::new(
            crate::error::ErrorKind::InvalidInput,
        )),
    }
}

impl AppState {
    /// Adapter for the request's profile, or the matching browser-safe error.
    pub fn adapter(
        &self,
        profile: Option<&str>,
    ) -> Result<Arc<dyn DatabaseAdapter>, crate::error::AppError> {
        use crate::error::{AppError, ErrorKind};
        match self.catalog_for(profile) {
            Lookup::Found(adapter) => Ok(adapter),
            Lookup::NotConfigured => Err(AppError::new(ErrorKind::DatabaseUnavailable)),
            Lookup::Unknown => Err(AppError::new(ErrorKind::UnknownProfile)),
        }
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
