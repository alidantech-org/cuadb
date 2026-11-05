//! Shared server-owned state. No mutable global connection registry.

use std::sync::{Arc, RwLock};

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

#[derive(Clone)]
struct Entry {
    /// A switch to another database of the same server; hidden from the connection list.
    derived: bool,
    /// Whether the SQL workspace may write through this entry.
    writable: bool,
    /// The connection URL (secret), kept only to derive same-server database switches.
    url: Option<crate::config::SecretUrl>,
    /// The database (or SQLite file) name; never a host, user or password.
    database: String,
    /// Saved from the Administration screen (editable); file-defined ones are read-only.
    managed: bool,
    id: String,
    label: String,
    runtime: Option<Runtime>,
    catalog: Arc<dyn DatabaseAdapter>,
    workspace: Option<Arc<dyn crate::sql::exec::Workspace>>,
}

/// Non-secret facts about a configured profile, safe for templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    /// Database or SQLite file name of this connection (no credentials).
    pub database: String,
    /// Saved from the Administration screen (can be edited or removed there).
    pub managed: bool,
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
    entries: Arc<RwLock<Vec<Entry>>>,
    default: usize,
    /// How to build connections at runtime; absent in test states.
    ctx: Option<Arc<BuildCtx>>,
    /// Seals saved connection secrets; absent when no `CUADB_SECRET_KEY` is configured.
    vault: Option<crate::secrets::SecretBox>,
    auth: Option<crate::auth::AuthService>,
    sql: SqlSettings,
    cancels: crate::sql::exec::CancelRegistry,
    txs: crate::sql::exec::TxRegistry,
}

#[derive(Clone)]
struct BuildCtx {
    limits: Limits,
    sqlite_root: std::path::PathBuf,
    sql_enabled: bool,
}

/// Most connections that may exist at once (file-defined plus saved).
pub const MAX_CONNECTIONS: usize = 16;

/// Database switches kept open at once; the oldest is closed beyond this.
const MAX_DERIVED: usize = 8;

/// Why a runtime connection change was refused. Never carries a URL or driver text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageError {
    /// This state cannot build connections (tests) or the id is not a saved connection.
    Unavailable,
    Exists,
    Unknown,
    TooMany,
    /// The address, path or options were rejected by the engine's URL validator.
    Invalid,
}

/// A saved connection ready to register. The URL is plaintext only inside this call.
pub struct ManagedInput {
    pub id: String,
    pub label: String,
    pub url: String,
    pub writable: bool,
}

/// Operator settings for the SQL workspace; everything is off unless enabled.
#[derive(Clone, Copy)]
pub struct SqlSettings {
    pub enabled: bool,
    pub max_rows: usize,
    pub query_timeout: std::time::Duration,
    pub tx_ttl: std::time::Duration,
}

impl Default for SqlSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_rows: 500,
            query_timeout: std::time::Duration::from_secs(15),
            tx_ttl: std::time::Duration::from_secs(60),
        }
    }
}

fn build_workspace(
    runtime: &Runtime,
    profile: &crate::profiles::Profile,
    limits: &Limits,
) -> Result<Option<Arc<dyn crate::sql::exec::Workspace>>, StateError> {
    Ok(match runtime {
        Runtime::PostgreSql(read) => {
            let write = if profile.writable {
                Some(
                    PgManager::with_access(profile.url.expose(), limits, true)
                        .map_err(StateError::PostgreSql)?
                        .pool(),
                )
            } else {
                None
            };
            Some(Arc::new(crate::db::postgres::workspace::PgWorkspace::new(
                read.pool(),
                write,
            )) as Arc<_>)
        }
        Runtime::Sqlite(manager) => Some(Arc::new(manager.workspace(profile.writable)) as Arc<_>),
        // MySQL/MariaDB workspace execution is deferred until the engine phase.
        Runtime::MySql(_) => None,
    })
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
            config
                .database_url()
                .cloned()
                .map(|url| ProfileSet::single(url, config.database_writable))
        };
        let Some(set) = set else {
            let mut state = Self::disconnected();
            state.ctx = Some(Arc::new(BuildCtx {
                limits: config.limits.clone(),
                sqlite_root: config.sqlite_root.clone(),
                sql_enabled: config.sql_workspace,
            }));
            state.sql = SqlSettings {
                enabled: config.sql_workspace,
                max_rows: config.sql_max_rows,
                query_timeout: config.limits.query_timeout,
                tx_ttl: std::time::Duration::from_secs(config.tx_timeout_seconds),
            };
            state.vault = config
                .secret_key
                .as_ref()
                .map(crate::secrets::SecretBox::new);
            return Ok(state);
        };
        // Divide the pool budget so many profiles cannot multiply open connections.
        let mut limits = config.limits.clone();
        limits.max_pool_connections =
            (limits.max_pool_connections / set.profiles.len() as u32).max(1);
        let mut entries = Vec::new();
        for profile in &set.profiles {
            let runtime = build_runtime(profile.url.expose(), &config.sqlite_root, &limits)?;
            let workspace = if config.sql_workspace {
                build_workspace(&runtime, profile, &limits)?
            } else {
                None
            };
            entries.push(Entry {
                derived: false,
                writable: profile.writable,
                url: Some(profile.url.clone()),
                database: database_name(profile.url.expose()),
                managed: false,
                id: profile.id.clone(),
                label: profile.label.clone(),
                catalog: runtime.catalog(),
                runtime: Some(runtime),
                workspace,
            });
        }
        Ok(Self {
            entries: Arc::new(RwLock::new(entries)),
            default: set.default,
            ctx: Some(Arc::new(BuildCtx {
                limits: config.limits.clone(),
                sqlite_root: config.sqlite_root.clone(),
                sql_enabled: config.sql_workspace,
            })),
            vault: config
                .secret_key
                .as_ref()
                .map(crate::secrets::SecretBox::new),
            auth: None,
            sql: SqlSettings {
                enabled: config.sql_workspace,
                max_rows: config.sql_max_rows,
                query_timeout: config.limits.query_timeout,
                tx_ttl: std::time::Duration::from_secs(config.tx_timeout_seconds),
            },
            cancels: Default::default(),
            txs: Default::default(),
        })
    }

    pub fn disconnected() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            ctx: None,
            default: 0,
            vault: None,
            auth: None,
            sql: SqlSettings::default(),
            cancels: Default::default(),
            txs: Default::default(),
        }
    }

    /// Serve an already-built adapter (for isolated router tests with fakes).
    /// It reports no connection runtime, so it never opens a database.
    pub fn with_adapter(adapter: Arc<dyn DatabaseAdapter>) -> Self {
        Self {
            ctx: None,
            entries: Arc::new(RwLock::new(vec![Entry {
                derived: false,
                writable: false,
                url: None,
                database: String::new(),
                managed: false,
                id: crate::profiles::DEFAULT_PROFILE_ID.into(),
                label: "Default connection".into(),
                runtime: None,
                catalog: adapter,
                workspace: None,
            }])),
            default: 0,
            vault: None,
            auth: None,
            sql: SqlSettings::default(),
            cancels: Default::default(),
            txs: Default::default(),
        }
    }

    /// Several fake adapters keyed by profile id, for isolated tests.
    pub fn with_adapters(adapters: Vec<(&str, &str, Arc<dyn DatabaseAdapter>)>) -> Self {
        Self {
            ctx: None,
            entries: Arc::new(RwLock::new(
                adapters
                    .into_iter()
                    .map(|(id, label, catalog)| Entry {
                        derived: false,
                        writable: false,
                        url: None,
                        database: String::new(),
                        managed: false,
                        id: id.into(),
                        label: label.into(),
                        runtime: None,
                        catalog,
                        workspace: None,
                    })
                    .collect(),
            )),
            default: 0,
            vault: None,
            auth: None,
            sql: SqlSettings::default(),
            cancels: Default::default(),
            txs: Default::default(),
        }
    }

    /// Roll back transactions that expired or whose session ended. Run periodically.
    pub async fn sweep_transactions(&self) -> usize {
        let mut alive = std::collections::HashSet::new();
        if let Some(auth) = self.auth() {
            for hash in self.txs.sessions().await {
                if auth.session_alive(&hash).await {
                    alive.insert(hash);
                }
            }
        }
        self.txs.sweep(|session| alive.contains(session)).await
    }

    pub fn sql_settings(&self) -> SqlSettings {
        self.sql
    }

    pub fn with_sql_settings(mut self, settings: SqlSettings) -> Self {
        self.sql = settings;
        self
    }

    pub fn cancels(&self) -> &crate::sql::exec::CancelRegistry {
        &self.cancels
    }

    pub fn txs(&self) -> &crate::sql::exec::TxRegistry {
        &self.txs
    }

    /// Workspace of a profile (None when disabled, unsupported or unknown).
    pub fn workspace_for(
        &self,
        profile: Option<&str>,
    ) -> Option<Arc<dyn crate::sql::exec::Workspace>> {
        self.entry(profile)
            .and_then(|entry| entry.workspace.clone())
    }

    /// Test helper: attach a workspace to an existing profile id.
    pub fn with_workspace(
        self,
        profile: &str,
        workspace: Arc<dyn crate::sql::exec::Workspace>,
    ) -> Self {
        if let Some(entry) = self.write().iter_mut().find(|e| e.id == profile) {
            entry.workspace = Some(workspace);
        }
        self
    }

    /// Attach the auth service (tests and startup).
    pub fn with_auth(mut self, auth: crate::auth::AuthService) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn auth(&self) -> Option<&crate::auth::AuthService> {
        self.auth.as_ref()
    }

    /// Open CuaDB's own state store when it exists (or when `CUADB_AUTH=required`).
    /// A store that exists but cannot be opened fails startup rather than silently
    /// running without authentication.
    pub async fn with_auth_from_config(self, config: &Config) -> Result<Self, StateError> {
        use crate::{
            auth::{AuthService, AuthStore, service::SessionPolicy, store::DB_FILE},
            config::AuthMode,
        };
        let required = config.auth_mode == AuthMode::Required;
        let present = config.data_dir.join(DB_FILE).is_file();
        if !present && !required {
            return Ok(self);
        }
        let store = AuthStore::open(&config.data_dir, required)
            .await
            .map_err(|e| StateError::Profiles(format!("authentication store: {e}")))?;
        let secure = config.cookie_secure.unwrap_or(!config.host.is_loopback());
        let policy = SessionPolicy {
            idle_secs: (config.session_idle_minutes * 60) as i64,
            max_secs: (config.session_max_hours * 3600) as i64,
            cookie_secure: secure,
        };
        let service = AuthService::new(store, policy, required)
            .await
            .map_err(|e| StateError::Profiles(format!("authentication store: {e}")))?;
        if required && service.store.active_admin_count().await.unwrap_or(0) == 0 {
            return Err(StateError::Profiles(
                "CUADB_AUTH=required but no administrator exists; run `cuadb admin bootstrap`"
                    .into(),
            ));
        }
        let state = self.with_auth(service);
        state.load_saved_connections().await?;
        Ok(state)
    }

    /// Register the connections saved from the Administration screen. Refuses to start
    /// when they exist but cannot be opened, rather than silently dropping them.
    async fn load_saved_connections(&self) -> Result<(), StateError> {
        let Some(auth) = self.auth() else {
            return Ok(());
        };
        let rows = auth
            .store
            .list_connections()
            .await
            .map_err(|_| StateError::Profiles("saved connections cannot be read".into()))?;
        if rows.is_empty() {
            return Ok(());
        }
        let Some(vault) = self.vault() else {
            return Err(StateError::Profiles(
                "saved connections exist but CUADB_SECRET_KEY is not set".into(),
            ));
        };
        for row in rows {
            let url = vault
                .open(&row.id, &row.url_enc)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .ok_or_else(|| {
                    StateError::Profiles(
                        "a saved connection cannot be decrypted; check CUADB_SECRET_KEY".into(),
                    )
                })?;
            self.add_managed(&ManagedInput {
                id: row.id,
                label: row.label,
                url,
                writable: row.writable,
            })
            .map_err(|_| StateError::Profiles("a saved connection is no longer valid".into()))?;
        }
        Ok(())
    }

    pub fn vault(&self) -> Option<&crate::secrets::SecretBox> {
        self.vault.as_ref()
    }

    /// Test helper: provide a vault without a configuration object.
    pub fn with_vault(mut self, vault: crate::secrets::SecretBox) -> Self {
        self.vault = Some(vault);
        self
    }

    /// The profile id a request refers to (`None` means the default profile).
    pub fn effective_profile(&self, profile: Option<&str>) -> String {
        let id = profile
            .map(str::to_string)
            .or_else(|| self.default_profile_id())
            .unwrap_or_else(|| crate::profiles::DEFAULT_PROFILE_ID.to_string());
        // A database switch authorizes as its base connection.
        match derived_parts(&id) {
            Some((base, _)) => base.to_string(),
            None => id,
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Vec<Entry>> {
        self.entries.read().unwrap_or_else(|p| p.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Vec<Entry>> {
        self.entries.write().unwrap_or_else(|p| p.into_inner())
    }

    fn default_entry(&self) -> Option<Entry> {
        self.read().get(self.default).cloned()
    }

    fn entry(&self, profile: Option<&str>) -> Option<Entry> {
        match profile {
            None => self.default_entry(),
            Some(id) => self.read().iter().find(|entry| entry.id == id).cloned(),
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
        self.read()
            .iter()
            .enumerate()
            .filter(|(_, entry)| !entry.derived)
            .map(|(index, entry)| ProfileInfo {
                database: entry.database.clone(),
                managed: entry.managed,
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

    pub fn default_profile_id(&self) -> Option<String> {
        self.default_entry().map(|entry| entry.id)
    }

    /// Optional startup probe of the default profile: errors change status, never crash.
    pub fn start_probe(&self) {
        if let Some(runtime) = self.default_entry().and_then(|e| e.runtime) {
            let _task = tokio::spawn(async move {
                let _ = runtime.snapshot().await;
            });
        }
    }

    pub async fn close(&self) {
        let entries = self.read().clone();
        for entry in entries {
            if let Some(runtime) = &entry.runtime {
                runtime.close().await;
            }
        }
    }

    /// Folder SQLite files must live in (None for test states).
    pub fn sqlite_root(&self) -> Option<std::path::PathBuf> {
        self.ctx.as_ref().map(|c| c.sqlite_root.clone())
    }

    /// Databases of the server behind `base` that its role may connect to.
    pub async fn databases_of(&self, base: &str) -> Result<Vec<String>, ManageError> {
        let entry = self.entry(Some(base)).ok_or(ManageError::Unknown)?;
        if entry.derived {
            return Err(ManageError::Unknown);
        }
        entry
            .catalog
            .list_databases()
            .await
            .map_err(|_| ManageError::Unavailable)
    }

    /// Register (or reuse) a switch to another database on the same server and return its
    /// profile id. The name must be one the connection's role may connect to.
    pub async fn use_database(&self, base: &str, database: &str) -> Result<String, ManageError> {
        let known = self.databases_of(base).await?;
        if !known.iter().any(|name| name == database) {
            return Err(ManageError::Unknown);
        }
        let id = derived_id(base, database);
        if self.entry(Some(&id)).is_some() {
            return Ok(id);
        }
        let base_entry = self.entry(Some(base)).ok_or(ManageError::Unknown)?;
        let url = base_entry.url.as_ref().ok_or(ManageError::Unavailable)?;
        let mut parsed = url::Url::parse(url.expose()).map_err(|_| ManageError::Invalid)?;
        if !matches!(parsed.scheme(), "postgres" | "postgresql") {
            return Err(ManageError::Unavailable);
        }
        parsed.set_path(&format!("/{database}"));
        let mut entry = self.build_entry(&ManagedInput {
            id: id.clone(),
            label: database.to_string(),
            url: parsed.into(),
            writable: base_entry.writable,
        })?;
        entry.managed = false;
        entry.derived = true;
        entry.database = database.to_string();
        let evicted = {
            let mut entries = self.write();
            let mut evicted = None;
            let derived = entries.iter().filter(|e| e.derived).count();
            if derived >= MAX_DERIVED
                && let Some(position) = entries.iter().position(|e| e.derived)
            {
                evicted = Some(entries.remove(position));
            }
            if !entries.iter().any(|e| e.id == id) {
                entries.push(entry);
            }
            evicted
        };
        if let Some(old) = evicted
            && let Some(runtime) = old.runtime
        {
            runtime.close().await;
        }
        Ok(id)
    }

    /// Whether this state can build connections at runtime.
    pub fn can_manage(&self) -> bool {
        self.ctx.is_some()
    }

    pub fn is_managed(&self, id: &str) -> bool {
        self.read().iter().any(|e| e.id == id && e.managed)
    }

    fn build_entry(&self, input: &ManagedInput) -> Result<Entry, ManageError> {
        let ctx = self.ctx.as_ref().ok_or(ManageError::Unavailable)?;
        let mut limits = ctx.limits.clone();
        // A saved connection never takes more than a quarter of the pool budget.
        limits.max_pool_connections = (limits.max_pool_connections / 4).max(1);
        let runtime = build_runtime(&input.url, &ctx.sqlite_root, &limits)
            .map_err(|_| ManageError::Invalid)?;
        let profile = crate::profiles::Profile {
            id: input.id.clone(),
            label: input.label.clone(),
            url: crate::config::SecretUrl::new(input.url.clone()),
            writable: input.writable,
        };
        let workspace = if ctx.sql_enabled {
            build_workspace(&runtime, &profile, &limits).map_err(|_| ManageError::Invalid)?
        } else {
            None
        };
        Ok(Entry {
            derived: false,
            writable: input.writable,
            url: Some(crate::config::SecretUrl::new(input.url.clone())),
            database: database_name(&input.url),
            managed: true,
            id: input.id.clone(),
            label: input.label.clone(),
            catalog: runtime.catalog(),
            runtime: Some(runtime),
            workspace,
        })
    }

    /// Register a saved connection. Pools open lazily, so this never contacts the database.
    pub fn add_managed(&self, input: &ManagedInput) -> Result<(), ManageError> {
        let entry = self.build_entry(input)?;
        let mut entries = self.write();
        if entries.len() >= MAX_CONNECTIONS {
            return Err(ManageError::TooMany);
        }
        if entries.iter().any(|e| e.id == input.id) {
            return Err(ManageError::Exists);
        }
        entries.push(entry);
        Ok(())
    }

    /// Replace a saved connection in place (same id); the old pools are closed.
    pub async fn replace_managed(&self, input: &ManagedInput) -> Result<(), ManageError> {
        let entry = self.build_entry(input)?;
        let old = {
            let mut entries = self.write();
            let slot = entries
                .iter_mut()
                .find(|e| e.id == input.id && e.managed)
                .ok_or(ManageError::Unknown)?;
            std::mem::replace(slot, entry)
        };
        if let Some(runtime) = old.runtime {
            runtime.close().await;
        }
        Ok(())
    }

    /// Remove a saved connection. File-defined connections are never removed.
    pub async fn remove_managed(&self, id: &str) -> Result<(), ManageError> {
        let removed = {
            let mut entries = self.write();
            let position = entries
                .iter()
                .position(|e| e.id == id && e.managed)
                .ok_or(ManageError::Unknown)?;
            entries.remove(position)
        };
        if let Some(runtime) = removed.runtime {
            runtime.close().await;
        }
        Ok(())
    }

    /// Open a throw-away connection and report its state, then close it.
    pub async fn test_url(&self, url: &str) -> Result<(ConnectionSnapshot, u128), ManageError> {
        let input = ManagedInput {
            id: "test".into(),
            label: "test".into(),
            url: url.to_string(),
            writable: false,
        };
        let entry = self.build_entry(&input)?;
        let runtime = entry.runtime.ok_or(ManageError::Unavailable)?;
        let started = std::time::Instant::now();
        let snapshot = tokio::time::timeout(std::time::Duration::from_secs(10), runtime.snapshot())
            .await
            .unwrap_or_else(|_| ConnectionSnapshot::unavailable());
        let elapsed = started.elapsed().as_millis();
        runtime.close().await;
        Ok((snapshot, elapsed))
    }

    pub fn is_configured(&self) -> bool {
        !self.read().is_empty()
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

/// `base~hex(database)` is a database switch on connection `base`.
pub fn derived_parts(id: &str) -> Option<(&str, String)> {
    let (base, hex) = id.split_once('~')?;
    if !crate::profiles::valid_id(base) || hex.is_empty() || hex.len() > 126 || hex.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    String::from_utf8(bytes?).ok().map(|db| (base, db))
}

pub fn derived_id(base: &str, database: &str) -> String {
    let hex: String = database.bytes().map(|b| format!("{b:02x}")).collect();
    format!("{base}~{hex}")
}

/// Validate the optional `profile` request parameter (syntax only).
pub fn checked_profile(value: Option<String>) -> Result<Option<String>, crate::error::AppError> {
    match value {
        None => Ok(None),
        Some(id) if crate::profiles::valid_id(&id) || derived_parts(&id).is_some() => Ok(Some(id)),
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

/// The database part of a connection URL, for display only.
fn database_name(url: &str) -> String {
    let Ok(parsed) = url::Url::parse(url) else {
        return String::new();
    };
    let last = parsed
        .path_segments()
        .and_then(|mut parts| parts.next_back().map(str::to_string))
        .unwrap_or_default();
    url::form_urlencoded::parse(format!("v={last}").as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
        .chars()
        .take(63)
        .collect()
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
    #[test]
    fn database_switch_ids_round_trip_and_reject_junk() {
        let id = derived_id("default", "my db-1");
        assert_eq!(derived_parts(&id), Some(("default", "my db-1".to_string())));
        assert!(checked_profile(Some(id)).is_ok());
        for bad in [
            "default~",
            "default~zz",
            "default~6",
            "Bad~6162",
            "~6162",
            "default~6162~6162",
        ] {
            assert!(derived_parts(bad).is_none(), "{bad}");
            assert!(checked_profile(Some(bad.into())).is_err(), "{bad}");
        }
        // A switch authorizes as its base connection.
        let state = AppState::disconnected();
        assert_eq!(
            state.effective_profile(Some(&derived_id("alpha", "x"))),
            "alpha"
        );
        assert_eq!(state.effective_profile(Some("alpha")), "alpha");
    }
}
