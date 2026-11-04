//! Execution contracts for the SQL workspace and typed edits, plus the registries
//! for cancellable statements and server-held transactions.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::sync::Notify;

use super::policy::DialectKind;
use crate::db::types::{Cell, DbError};

#[derive(Debug, Clone, Copy)]
pub struct ExecLimits {
    pub timeout: Duration,
    pub max_rows: usize,
    pub max_bytes: usize,
}

impl Default for ExecLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            max_rows: 500,
            max_bytes: 2 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    /// Rows or bytes were cut off at the limits.
    pub truncated: bool,
    /// Rows changed by a write statement.
    pub affected: Option<u64>,
    pub elapsed_ms: u64,
}

/// Lets a running statement be cancelled from another request.
#[derive(Default)]
pub struct CancelHandle {
    notify: Notify,
    cancelled: Mutex<bool>,
    /// Backend/session id on engines that can cancel server-side.
    pub backend_pid: Mutex<Option<i64>>,
}

impl CancelHandle {
    pub fn cancel(&self) {
        *self.cancelled.lock().unwrap_or_else(|p| p.into_inner()) = true;
        self.notify.notify_waiters();
        self.notify.notify_one();
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}

#[async_trait::async_trait]
pub trait Workspace: Send + Sync {
    fn dialect(&self) -> DialectKind;
    /// Whether a writer pool exists (profile access `read-write`).
    fn writable(&self) -> bool;
    /// Run an already-classified read statement in a read-only transaction.
    async fn query(
        &self,
        sql: &str,
        limits: &ExecLimits,
        cancel: &CancelHandle,
    ) -> Result<SqlResult, DbError>;
    /// Open a write transaction on the writer pool.
    async fn begin(&self, limits: &ExecLimits) -> Result<Box<dyn WriteTx>, DbError>;
}

#[async_trait::async_trait]
pub trait WriteTx: Send {
    /// Execute a statement with text parameters (`$1`/`?` placeholders), returning rows affected.
    async fn execute(&mut self, sql: &str, binds: &[Option<String>]) -> Result<u64, DbError>;
    /// Run a read inside the transaction (sees its own uncommitted changes).
    async fn query(&mut self, sql: &str, max_rows: usize) -> Result<SqlResult, DbError>;
    async fn commit(self: Box<Self>) -> Result<(), DbError>;
    async fn rollback(self: Box<Self>) -> Result<(), DbError>;
}

const MAX_RUNNING: usize = 64;

type RunningStatements = HashMap<String, (Option<i64>, Arc<CancelHandle>)>;

/// Statements currently running, keyed by an unguessable id, owned by one user.
#[derive(Default, Clone)]
pub struct CancelRegistry {
    inner: Arc<Mutex<RunningStatements>>,
}

impl CancelRegistry {
    pub fn register(&self, owner: Option<i64>) -> Option<(String, Arc<CancelHandle>)> {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if map.len() >= MAX_RUNNING {
            return None;
        }
        let id = crate::auth::session::new_token()[..32].to_string();
        let handle = Arc::new(CancelHandle::default());
        map.insert(id.clone(), (owner, handle.clone()));
        Some((id, handle))
    }

    /// Register under a client-chosen id (32 hex characters) so the page can cancel
    /// the request it is still waiting for. Ids cannot be reused while running.
    pub fn register_with(&self, id: &str, owner: Option<i64>) -> Option<Arc<CancelHandle>> {
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if map.len() >= MAX_RUNNING || map.contains_key(id) {
            return None;
        }
        let handle = Arc::new(CancelHandle::default());
        map.insert(id.to_string(), (owner, handle.clone()));
        Some(handle)
    }

    pub fn finish(&self, id: &str) {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
    }

    /// Cancel only a statement the caller owns.
    pub fn cancel(&self, id: &str, owner: Option<i64>) -> bool {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        match map.get(id) {
            Some((o, handle)) if *o == owner => {
                handle.cancel();
                true
            }
            _ => false,
        }
    }
}

/// One change staged in an open transaction, described without values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingChange {
    pub summary: String,
    pub affected: u64,
}

pub struct TxEntry {
    pub owner: i64,
    pub session: Vec<u8>,
    pub profile: String,
    pub started: Instant,
    pub deadline: Instant,
    pub pending: Vec<PendingChange>,
    pub tx: Option<Box<dyn WriteTx>>,
}

pub const MAX_TX_PER_USER: usize = 2;
pub const MAX_PENDING_CHANGES: usize = 200;

/// Server-held write transactions. Entries are removed (and rolled back by
/// dropping the connection) on commit, rollback, expiry, logout or disable.
#[derive(Default, Clone)]
pub struct TxRegistry {
    inner: Arc<tokio::sync::Mutex<HashMap<String, TxEntry>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxError {
    TooMany,
    /// The user already has an open transaction on another profile.
    OtherProfile,
    NotFound,
    Expired,
    TooManyChanges,
}

impl TxRegistry {
    pub async fn begin(
        &self,
        owner: i64,
        session: &[u8],
        profile: &str,
        ttl: Duration,
        tx: Box<dyn WriteTx>,
    ) -> Result<String, TxError> {
        let mut map = self.inner.lock().await;
        let mine: Vec<&TxEntry> = map.values().filter(|e| e.owner == owner).collect();
        if mine.iter().any(|e| e.profile != profile) {
            return Err(TxError::OtherProfile);
        }
        if mine.len() >= MAX_TX_PER_USER {
            return Err(TxError::TooMany);
        }
        let id = crate::auth::session::new_token()[..32].to_string();
        let now = Instant::now();
        map.insert(
            id.clone(),
            TxEntry {
                owner,
                session: session.to_vec(),
                profile: profile.to_string(),
                started: now,
                deadline: now + ttl,
                pending: vec![],
                tx: Some(tx),
            },
        );
        Ok(id)
    }

    /// Session hashes that currently hold transactions (for expiry sweeps).
    pub async fn sessions(&self) -> Vec<Vec<u8>> {
        let map = self.inner.lock().await;
        let mut all: Vec<Vec<u8>> = map.values().map(|e| e.session.clone()).collect();
        all.sort();
        all.dedup();
        all
    }

    /// Profile of the caller's open transaction, if any (used to refuse switching).
    pub async fn open_profile(&self, owner: i64) -> Option<String> {
        let map = self.inner.lock().await;
        map.values()
            .find(|e| e.owner == owner)
            .map(|e| e.profile.clone())
    }

    /// Take the transaction out for one operation; put it back with `restore`.
    pub async fn checkout(
        &self,
        id: &str,
        owner: i64,
    ) -> Result<(Box<dyn WriteTx>, String), TxError> {
        let mut map = self.inner.lock().await;
        let entry = map
            .get_mut(id)
            .filter(|e| e.owner == owner)
            .ok_or(TxError::NotFound)?;
        if Instant::now() >= entry.deadline {
            map.remove(id);
            return Err(TxError::Expired);
        }
        let profile = entry.profile.clone();
        let tx = entry.tx.take().ok_or(TxError::NotFound)?;
        Ok((tx, profile))
    }

    pub async fn restore(
        &self,
        id: &str,
        tx: Box<dyn WriteTx>,
        change: Option<PendingChange>,
    ) -> Result<(), TxError> {
        let mut map = self.inner.lock().await;
        let Some(entry) = map.get_mut(id) else {
            return Err(TxError::NotFound);
        };
        entry.tx = Some(tx);
        if let Some(change) = change {
            if entry.pending.len() >= MAX_PENDING_CHANGES {
                return Err(TxError::TooManyChanges);
            }
            entry.pending.push(change);
        }
        Ok(())
    }

    pub async fn pending(
        &self,
        id: &str,
        owner: i64,
    ) -> Result<(String, Vec<PendingChange>, Duration), TxError> {
        let map = self.inner.lock().await;
        let entry = map
            .get(id)
            .filter(|e| e.owner == owner)
            .ok_or(TxError::NotFound)?;
        Ok((
            entry.profile.clone(),
            entry.pending.clone(),
            entry.deadline.saturating_duration_since(Instant::now()),
        ))
    }

    /// Remove an entry whose transaction was already taken out (after a failed step).
    pub async fn discard(&self, id: &str, owner: i64) {
        let mut map = self.inner.lock().await;
        if map.get(id).is_some_and(|e| e.owner == owner) {
            map.remove(id);
        }
    }

    /// Remove an entry for commit or rollback.
    pub async fn take(&self, id: &str, owner: i64) -> Result<(TxEntry, Box<dyn WriteTx>), TxError> {
        let mut map = self.inner.lock().await;
        if !map.get(id).is_some_and(|e| e.owner == owner) {
            return Err(TxError::NotFound);
        }
        let mut entry = map.remove(id).ok_or(TxError::NotFound)?;
        let tx = entry.tx.take().ok_or(TxError::NotFound)?;
        Ok((entry, tx))
    }

    /// Roll back everything expired or whose session is no longer alive.
    /// Returns how many were rolled back.
    pub async fn sweep(&self, alive: impl Fn(&[u8]) -> bool) -> usize {
        let doomed: Vec<TxEntry> = {
            let mut map = self.inner.lock().await;
            let now = Instant::now();
            let ids: Vec<String> = map
                .iter()
                .filter(|(_, e)| now >= e.deadline || !alive(&e.session))
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter().filter_map(|id| map.remove(&id)).collect()
        };
        let count = doomed.len();
        for mut entry in doomed {
            if let Some(tx) = entry.tx.take() {
                let _ = tx.rollback().await;
            }
        }
        count
    }

    /// Roll back every transaction of one user (logout, disable).
    pub async fn rollback_user(&self, owner: i64) -> usize {
        let doomed: Vec<TxEntry> = {
            let mut map = self.inner.lock().await;
            let ids: Vec<String> = map
                .iter()
                .filter(|(_, e)| e.owner == owner)
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter().filter_map(|id| map.remove(&id)).collect()
        };
        let count = doomed.len();
        for mut entry in doomed {
            if let Some(tx) = entry.tx.take() {
                let _ = tx.rollback().await;
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Noop(Arc<Mutex<Vec<&'static str>>>);

    #[async_trait::async_trait]
    impl WriteTx for Noop {
        async fn execute(&mut self, _: &str, _: &[Option<String>]) -> Result<u64, DbError> {
            Ok(1)
        }
        async fn query(&mut self, _: &str, _: usize) -> Result<SqlResult, DbError> {
            Err(DbError::new(crate::db::types::DbErrorKind::Unsupported))
        }
        async fn commit(self: Box<Self>) -> Result<(), DbError> {
            self.0.lock().unwrap().push("commit");
            Ok(())
        }
        async fn rollback(self: Box<Self>) -> Result<(), DbError> {
            self.0.lock().unwrap().push("rollback");
            Ok(())
        }
    }

    fn noop(log: &Arc<Mutex<Vec<&'static str>>>) -> Box<dyn WriteTx> {
        Box::new(Noop(log.clone()))
    }

    #[tokio::test]
    async fn transactions_are_owned_capped_single_profile_and_swept() {
        let log = Arc::new(Mutex::new(vec![]));
        let reg = TxRegistry::default();
        let ttl = Duration::from_secs(60);
        let a = reg.begin(1, b"s", "alpha", ttl, noop(&log)).await.unwrap();
        assert_eq!(
            reg.begin(1, b"s", "beta", ttl, noop(&log)).await,
            Err(TxError::OtherProfile)
        );
        reg.begin(1, b"s", "alpha", ttl, noop(&log)).await.unwrap();
        assert_eq!(
            reg.begin(1, b"s", "alpha", ttl, noop(&log)).await,
            Err(TxError::TooMany)
        );
        // Another user cannot see or use it.
        assert!(matches!(reg.checkout(&a, 2).await, Err(TxError::NotFound)));
        assert!(matches!(reg.pending(&a, 2).await, Err(TxError::NotFound)));
        assert_eq!(reg.open_profile(1).await.as_deref(), Some("alpha"));
        assert_eq!(reg.open_profile(2).await, None);
        // Logout of the session rolls both back.
        assert_eq!(reg.sweep(|_| false).await, 2);
        assert_eq!(log.lock().unwrap().as_slice(), ["rollback", "rollback"]);
        assert_eq!(reg.open_profile(1).await, None);
    }

    #[tokio::test]
    async fn expired_transactions_are_rolled_back_and_refused() {
        let log = Arc::new(Mutex::new(vec![]));
        let reg = TxRegistry::default();
        let id = reg
            .begin(7, b"s", "alpha", Duration::from_millis(1), noop(&log))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(matches!(reg.checkout(&id, 7).await, Err(TxError::Expired)));
        let id2 = reg
            .begin(7, b"s", "alpha", Duration::from_millis(1), noop(&log))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(reg.sweep(|_| true).await, 1);
        assert!(matches!(reg.pending(&id2, 7).await, Err(TxError::NotFound)));
        assert_eq!(log.lock().unwrap().as_slice(), ["rollback"]);
    }

    #[tokio::test]
    async fn cancel_handles_only_obey_their_owner() {
        let reg = CancelRegistry::default();
        let (id, handle) = reg.register(Some(1)).unwrap();
        assert!(!reg.cancel(&id, Some(2)) && !handle.is_cancelled());
        assert!(!reg.cancel("nope", Some(1)));
        assert!(reg.cancel(&id, Some(1)) && handle.is_cancelled());
        handle.cancelled().await;
        reg.finish(&id);
        assert!(!reg.cancel(&id, Some(1)));
    }
}
