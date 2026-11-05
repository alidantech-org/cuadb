//! Restricted, read-only SQLite adapter.
use crate::{
    config::Limits,
    db::types::{ConnectionSnapshot, ConnectionStatus, DbError, DbErrorKind, Engine},
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::time::timeout;
pub mod catalog;
pub mod workspace;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteUrlError {
    Scheme,
    Path,
    Root,
}
impl std::fmt::Display for SqliteUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Scheme => "SQLite URL must use sqlite:///absolute/path",
            Self::Path => "SQLite path must be an existing regular file",
            Self::Root => "SQLite path must remain within CUADB_SQLITE_ROOT",
        })
    }
}
impl std::error::Error for SqliteUrlError {}
pub fn resolve_path(raw: &str, root: &Path) -> Result<PathBuf, SqliteUrlError> {
    let value = raw
        .strip_prefix("sqlite:///")
        .ok_or(SqliteUrlError::Scheme)?;
    if value.contains('?') || value.contains('#') || value.as_bytes().contains(&0) {
        return Err(SqliteUrlError::Path);
    }
    let path = Path::new("/").join(value);
    let canonical_root = root.canonicalize().map_err(|_| SqliteUrlError::Root)?;
    let canonical = path.canonicalize().map_err(|_| SqliteUrlError::Path)?;
    if !canonical.starts_with(&canonical_root) {
        return Err(SqliteUrlError::Root);
    }
    if !canonical.is_file() {
        return Err(SqliteUrlError::Path);
    }
    Ok(canonical)
}
#[derive(Clone)]
pub struct SqliteManager {
    pool: SqlitePool,
    path: PathBuf,
    limits: Limits,
}
impl SqliteManager {
    pub fn new(raw: &str, root: &Path, limits: &Limits) -> Result<Self, SqliteUrlError> {
        let path = resolve_path(raw, root)?;
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .read_only(true)
            .create_if_missing(false)
            .busy_timeout(limits.connection_timeout);
        let pool = SqlitePoolOptions::new()
            .max_connections(limits.max_pool_connections)
            .min_connections(0)
            .acquire_timeout(limits.connection_timeout)
            .idle_timeout(Some(Duration::from_secs(120)))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    sqlx::query("PRAGMA query_only=ON")
                        .execute(&mut *connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_lazy_with(options);
        Ok(Self {
            pool,
            path,
            limits: limits.clone(),
        })
    }
    /// SQL workspace pools for this file. The writer opens the file read-write and is
    /// created only for profiles explicitly marked `read-write`.
    pub fn workspace(&self, writable: bool) -> workspace::SqliteWorkspace {
        workspace::SqliteWorkspace::new(&self.path, writable, &self.limits)
    }

    pub fn catalog(&self) -> catalog::SqliteCatalog {
        catalog::SqliteCatalog::new(
            self.pool.clone(),
            self.limits.query_timeout,
            self.limits.max_page_size,
        )
    }
    pub async fn snapshot(&self) -> ConnectionSnapshot {
        let result = timeout(
            self.limits.connection_timeout + self.limits.query_timeout,
            sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&self.pool),
        )
        .await;
        match result {
            Ok(Ok(_)) => ConnectionSnapshot {
                engine: Engine::Sqlite,
                status: ConnectionStatus::Ready,
                error_code: None,
            },
            _ => ConnectionSnapshot {
                engine: Engine::Sqlite,
                status: ConnectionStatus::Unavailable,
                error_code: Some("database_unavailable"),
            },
        }
    }
    pub async fn close(&self) {
        self.pool.close().await;
    }
}
pub(crate) fn classify(error: sqlx::Error) -> DbError {
    match error {
        sqlx::Error::PoolTimedOut => DbError::new(DbErrorKind::TimedOut),
        sqlx::Error::Database(_) => DbError::new(DbErrorKind::Internal),
        _ => DbError::new(DbErrorKind::ConnectionUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn readonly_pool_rejects_mutations() {
        let root = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sqlite-data"
        ));
        let url = concat!(
            "sqlite:///",
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sqlite-data/cuadb_fixture_sqlite.db"
        );
        let limits = Limits {
            connection_timeout: Duration::from_secs(2),
            query_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(2),
            default_page_size: 100,
            max_page_size: 1000,
            max_pool_connections: 1,
            max_in_flight_requests: 1,
            max_request_body_bytes: 1024,
        };
        let manager = SqliteManager::new(url, root, &limits).unwrap();
        let result = sqlx::query("INSERT INTO items (id, name) VALUES (999999, 'forbidden')")
            .execute(&manager.pool)
            .await;
        assert!(result.is_err(), "read-only SQLite pool accepted INSERT");
        manager.close().await;
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_resolve_outside_the_allowed_root() {
        use std::{fs, os::unix::fs::symlink, time::SystemTime};
        let root = std::env::temp_dir().join(format!(
            "cuadb-symlink-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let escape = root.join("outside.db");
        symlink("/etc/passwd", &escape).unwrap();
        let url = format!("sqlite:///{}", escape.display());
        assert_eq!(resolve_path(&url, &root), Err(SqliteUrlError::Root));
        fs::remove_file(escape).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
