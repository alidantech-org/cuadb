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
    /// Seals saved connection secrets with the configured or data-directory key.
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
static DEMO_DATABASE_INIT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

#[derive(Clone, Copy)]
pub enum DatabaseOperation<'a> {
    CreateDatabase(&'a str),
    DropDatabase(&'a str),
    DropSchema(&'a str),
    DropTable(&'a str, &'a str),
}

fn quote_lifecycle_identifier(name: &str) -> Result<String, ManageError> {
    if name.is_empty() || name.len() > 63 || name.chars().any(char::is_control) {
        return Err(ManageError::Invalid);
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
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
    /// Trusted local instance: administrator-level access without sign-in.
    pub local: bool,
    pub max_rows: usize,
    pub query_timeout: std::time::Duration,
    pub tx_ttl: std::time::Duration,
}

impl Default for SqlSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            local: false,
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

fn configured_vault(config: &Config) -> Result<crate::secrets::SecretBox, StateError> {
    let key = if let Some(key) = &config.secret_key {
        key.clone()
    } else {
        let directory = &config.data_dir;
        std::fs::create_dir_all(directory).map_err(|_| {
            StateError::Profiles("the application data directory is unavailable".into())
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).map_err(
                |_| {
                    StateError::Profiles(
                        "the application data directory permissions could not be secured".into(),
                    )
                },
            )?;
        }
        let path = directory.join("connection.key");
        let saved = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let generated = crate::secrets::SecretKey::generate().ok_or_else(|| {
                    StateError::Profiles(
                        "a connection encryption key could not be generated".into(),
                    )
                })?;
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                match options.open(&path) {
                    Ok(mut file) => {
                        use std::io::Write;
                        file.write_all(generated.to_hex().as_bytes())
                            .and_then(|()| file.sync_all())
                            .map_err(|_| {
                                StateError::Profiles(
                                    "the connection encryption key could not be saved".into(),
                                )
                            })?;
                        generated.to_hex()
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        std::fs::read_to_string(&path).map_err(|_| {
                            StateError::Profiles(
                                "the connection encryption key could not be read".into(),
                            )
                        })?
                    }
                    Err(_) => {
                        return Err(StateError::Profiles(
                            "the connection encryption key could not be saved".into(),
                        ));
                    }
                }
            }
            Err(_) => {
                return Err(StateError::Profiles(
                    "the connection encryption key could not be read".into(),
                ));
            }
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(
                |_| {
                    StateError::Profiles(
                        "the connection encryption key permissions could not be secured".into(),
                    )
                },
            )?;
        }
        crate::secrets::SecretKey::parse_hex(&saved).ok_or_else(|| {
            StateError::Profiles("the saved connection encryption key is invalid".into())
        })?
    };
    Ok(crate::secrets::SecretBox::new(&key))
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
    pub async fn from_config(config: &Config) -> Result<Self, StateError> {
        let vault = configured_vault(config)?;
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
                local: config.local_mode,
                max_rows: config.sql_max_rows,
                query_timeout: config.limits.query_timeout,
                tx_ttl: std::time::Duration::from_secs(config.tx_timeout_seconds),
            };
            state.vault = Some(vault.clone());
            state.add_demo_database(config).await?;
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
        let mut state = Self {
            entries: Arc::new(RwLock::new(entries)),
            default: set.default,
            ctx: Some(Arc::new(BuildCtx {
                limits: config.limits.clone(),
                sqlite_root: config.sqlite_root.clone(),
                sql_enabled: config.sql_workspace,
            })),
            vault: Some(vault),
            auth: None,
            sql: SqlSettings {
                enabled: config.sql_workspace,
                local: config.local_mode,
                max_rows: config.sql_max_rows,
                query_timeout: config.limits.query_timeout,
                tx_ttl: std::time::Duration::from_secs(config.tx_timeout_seconds),
            },
            cancels: Default::default(),
            txs: Default::default(),
        };
        state.add_demo_database(config).await?;
        Ok(state)
    }

    async fn add_demo_database(&mut self, config: &Config) -> Result<(), StateError> {
        let _initialization = DEMO_DATABASE_INIT.lock().await;
        std::fs::create_dir_all(&config.sqlite_root)
            .map_err(|_| StateError::Profiles("the SQLite data folder is unavailable".into()))?;
        let root = std::fs::canonicalize(&config.sqlite_root)
            .map_err(|_| StateError::Profiles("the SQLite data folder is unavailable".into()))?;
        let path = root.join("cuadb-demo.db");
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|_| {
                StateError::Profiles("the demo SQLite database could not be created".into())
            })?;
        let url = format!(
            "sqlite:///{}",
            path.display().to_string().trim_start_matches('/')
        );
        let runtime = build_runtime(&url, &config.sqlite_root, &config.limits)?;
        let Runtime::Sqlite(manager) = &runtime else {
            return Err(StateError::Profiles(
                "the demo SQLite database could not be opened".into(),
            ));
        };
        let writer = manager.workspace(true);
        let limits = crate::sql::exec::ExecLimits::default();
        let initialized = async {
            use crate::sql::exec::Workspace;
            let mut tx = writer.begin(&limits).await.map_err(|_| ())?;
            for statement in [
                "CREATE TABLE IF NOT EXISTS projects (id INTEGER PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL)",
                "INSERT OR IGNORE INTO projects (id, name, status, created_at) VALUES (1, 'CuaDB sample', 'ready', '2025-01-01')",
                "CREATE TABLE IF NOT EXISTS tasks (id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL REFERENCES projects(id), title TEXT NOT NULL, complete INTEGER NOT NULL)",
                "INSERT OR IGNORE INTO tasks (id, project_id, title, complete) VALUES (1, 1, 'Explore a table', 0)",
            ] {
                tx.execute(statement, &[]).await.map_err(|_| ())?;
            }
            tx.commit().await.map_err(|_| ())
        };
        initialized.await.map_err(|_| {
            StateError::Profiles("the demo database could not be initialized".into())
        })?;
        let workspace = if config.sql_workspace {
            let profile = crate::profiles::Profile {
                id: "demo".into(),
                label: "Demo database".into(),
                url: crate::config::SecretUrl::new(url.clone()),
                writable: false,
            };
            build_workspace(&runtime, &profile, &config.limits)?
        } else {
            None
        };
        let demo = Entry {
            derived: false,
            writable: false,
            url: Some(crate::config::SecretUrl::new(url)),
            database: "cuadb-demo".into(),
            managed: true,
            id: "demo".into(),
            label: "Demo database".into(),
            catalog: runtime.catalog(),
            runtime: Some(runtime),
            workspace,
        };
        let (old, was_empty) = {
            let mut entries = self.write();
            if let Some(index) = entries.iter().position(|entry| entry.id == "demo") {
                (Some(std::mem::replace(&mut entries[index], demo)), false)
            } else {
                let was_empty = entries.is_empty();
                entries.push(demo);
                (None, was_empty)
            }
        };
        if was_empty {
            self.default = 0;
        }
        if let Some(runtime) = old.and_then(|entry| entry.runtime) {
            runtime.close().await;
        }
        Ok(())
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
        if self.local_mode() {
            // The trusted local instance has one built-in session with no stored hash.
            alive.insert(Vec::new());
        }
        if let Some(auth) = self.auth() {
            for hash in self.txs.sessions().await {
                if auth.session_alive(&hash).await {
                    alive.insert(hash);
                }
            }
        }
        self.txs.sweep(|session| alive.contains(session)).await
    }

    pub fn local_mode(&self) -> bool {
        self.sql.local
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

    /// Open or create CuaDB's own store for user state and saved connections.
    pub async fn with_auth_from_config(self, config: &Config) -> Result<Self, StateError> {
        use crate::{
            auth::{AuthService, AuthStore, service::SessionPolicy},
            config::AuthMode,
        };
        let required = config.auth_mode == AuthMode::Required;
        let store = AuthStore::open(&config.data_dir, true)
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
            let is_demo = row.id == "demo";
            let id = row.id.clone();
            let Some(url) = vault
                .open(&row.id, &row.url_enc)
                .and_then(|bytes| String::from_utf8(bytes).ok())
            else {
                eprintln!("level=warn event=saved_connection_skipped id={id} reason=undecryptable");
                continue;
            };
            let input = ManagedInput {
                id: row.id,
                label: row.label,
                url,
                writable: row.writable,
            };
            let loaded = if is_demo {
                self.replace_demo_connection(&input).await.is_ok()
            } else {
                self.add_managed(&input).is_ok()
            };
            if !loaded {
                eprintln!("level=warn event=saved_connection_skipped id={id} reason=invalid");
            }
        }
        Ok(())
    }

    async fn replace_demo_connection(&self, input: &ManagedInput) -> Result<(), StateError> {
        let entry = self.build_entry(input).map_err(|_| {
            StateError::Profiles("the saved demo connection is no longer valid".into())
        })?;
        let old = {
            let mut entries = self.write();
            if let Some(slot) = entries.iter_mut().find(|entry| entry.id == "demo") {
                Some(std::mem::replace(slot, entry))
            } else {
                if entries.len() >= MAX_CONNECTIONS {
                    return Err(StateError::Profiles(
                        "the saved connection limit has been reached".into(),
                    ));
                }
                entries.push(entry);
                None
            }
        };
        if let Some(old) = old
            && let Some(runtime) = old.runtime
        {
            runtime.close().await;
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

    pub fn public_principal(&self) -> crate::auth::Principal {
        let profiles = self.profiles();
        crate::auth::Principal::public_connections(
            profiles.into_iter().map(|profile| (profile.id, false)),
        )
    }

    pub fn public_managed_principal(&self) -> crate::auth::Principal {
        crate::auth::Principal::public_connections(
            self.profiles()
                .into_iter()
                .filter(|profile| profile.managed && profile.id != "demo")
                .map(|profile| (profile.id, true)),
        )
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

    pub async fn database_sizes_of(&self, base: &str) -> Result<Vec<(String, u64)>, ManageError> {
        let entry = self.entry(Some(base)).ok_or(ManageError::Unknown)?;
        if entry.derived {
            return Err(ManageError::Unknown);
        }
        entry
            .catalog
            .database_sizes()
            .await
            .map_err(|_| ManageError::Unavailable)
    }

    /// Execute a narrowly scoped PostgreSQL lifecycle command. Names are quoted here, and the
    /// caller must separately authorize the profile-wide SQL capability.
    pub async fn database_lifecycle(
        &self,
        base: &str,
        operation: DatabaseOperation<'_>,
    ) -> Result<(), ManageError> {
        let entry = self.entry(Some(base)).ok_or(ManageError::Unknown)?;
        if !entry.writable {
            return Err(ManageError::Unavailable);
        }
        let raw = entry.url.as_ref().ok_or(ManageError::Unavailable)?.expose();
        if !raw.starts_with("postgres://") && !raw.starts_with("postgresql://") {
            return Err(ManageError::Unavailable);
        }
        let ctx = self.ctx.as_ref().ok_or(ManageError::Unavailable)?;
        let (url, statement) = match operation {
            DatabaseOperation::CreateDatabase(name) | DatabaseOperation::DropDatabase(name) => {
                let name = quote_lifecycle_identifier(name)?;
                let mut url = url::Url::parse(raw).map_err(|_| ManageError::Invalid)?;
                url.set_path("/postgres");
                let statement = if matches!(operation, DatabaseOperation::CreateDatabase(_)) {
                    format!("CREATE DATABASE {name}")
                } else {
                    format!("DROP DATABASE {name} WITH (FORCE)")
                };
                (url, statement)
            }
            DatabaseOperation::DropSchema(schema) => {
                let schema = quote_lifecycle_identifier(schema)?;
                (
                    url::Url::parse(raw).map_err(|_| ManageError::Invalid)?,
                    format!("DROP SCHEMA {schema} CASCADE"),
                )
            }
            DatabaseOperation::DropTable(schema, table) => {
                let schema = quote_lifecycle_identifier(schema)?;
                let table = quote_lifecycle_identifier(table)?;
                (
                    url::Url::parse(raw).map_err(|_| ManageError::Invalid)?,
                    format!("DROP TABLE {schema}.{table} CASCADE"),
                )
            }
        };
        let manager = crate::db::postgres::PgManager::with_access(url.as_str(), &ctx.limits, true)
            .map_err(|_| ManageError::Invalid)?;
        let result = sqlx::query(&statement).execute(&manager.pool()).await;
        manager.close().await;
        result.map(|_| ()).map_err(|_| ManageError::Unavailable)
    }

    /// Run schema-changing statements in one transaction on a writable connection of a
    /// writable profile. The error text is the database's own message.
    pub async fn run_ddl(
        &self,
        base: &str,
        statements: &[String],
        server_level: bool,
    ) -> Result<(), String> {
        let entry = self
            .entry(Some(base))
            .ok_or_else(|| "unknown connection".to_string())?;
        if !entry.writable {
            return Err("this connection is read-only; enable changes on it first".into());
        }
        match entry.runtime.as_ref() {
            Some(Runtime::Sqlite(manager)) => manager.execute_ddl(statements).await,
            Some(Runtime::PostgreSql(_)) => {
                let raw = entry
                    .url
                    .as_ref()
                    .ok_or_else(|| "connection is unavailable".to_string())?
                    .expose();
                let ctx = self
                    .ctx
                    .as_ref()
                    .ok_or_else(|| "connection is unavailable".to_string())?;
                let mut url = raw.to_string();
                if server_level {
                    let mut parsed =
                        url::Url::parse(raw).map_err(|_| "connection is invalid".to_string())?;
                    parsed.set_path("/postgres");
                    url = parsed.into();
                }
                let manager = crate::db::postgres::PgManager::with_access(&url, &ctx.limits, true)
                    .map_err(|_| "connection is invalid".to_string())?;
                let result = async {
                    if server_level {
                        // CREATE/DROP DATABASE cannot run inside a transaction.
                        for statement in statements {
                            sqlx::query(statement).execute(&manager.pool()).await?;
                        }
                        return Ok(());
                    }
                    let mut tx = manager.pool().begin().await?;
                    for statement in statements {
                        sqlx::query(statement).execute(&mut *tx).await?;
                    }
                    tx.commit().await
                }
                .await
                .map_err(|error| match error {
                    sqlx::Error::Database(db) => db.message().to_string(),
                    other => other.to_string(),
                });
                manager.close().await;
                result
            }
            _ => Err("this engine does not support schema changes here".into()),
        }
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

    pub async fn reconnect(&self, id: &str) -> Result<(), ManageError> {
        let old = self.entry(Some(id)).ok_or(ManageError::Unknown)?;
        let url = old
            .url
            .as_ref()
            .ok_or(ManageError::Unavailable)?
            .expose()
            .to_string();
        let replacement = self.build_entry(&ManagedInput {
            id: old.id.clone(),
            label: old.label.clone(),
            url,
            writable: old.writable,
        })?;
        let old = {
            let mut entries = self.write();
            let slot = entries
                .iter_mut()
                .find(|entry| entry.id == id)
                .ok_or(ManageError::Unknown)?;
            std::mem::replace(slot, replacement)
        };
        if let Some(runtime) = old.runtime {
            runtime.close().await;
        }
        Ok(())
    }

    pub async fn disconnect_profile(&self, id: &str) -> Result<(), ManageError> {
        let old = {
            let mut entries = self.write();
            let slot = entries
                .iter_mut()
                .find(|entry| entry.id == id)
                .ok_or(ManageError::Unknown)?;
            if slot.runtime.is_none() {
                return Ok(());
            }
            let old_runtime = slot.runtime.take();
            slot.runtime = None;
            old_runtime
        };
        if let Some(runtime) = old {
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
        let state = AppState::from_config(&config).await.unwrap();
        assert!(state.is_configured());
        assert_eq!(state.profiles()[0].id, "demo");
        state.close().await;
    }

    #[tokio::test]
    async fn malformed_connection_url_fails_at_startup_without_echoing_password() {
        let config = Config::from_values(&[(
            "CUADB_DATABASE_URL",
            "postgresql://viewer:secret@db/db?options=-c+bad",
        )])
        .unwrap();
        let outcome = AppState::from_config(&config).await.err().unwrap();
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
