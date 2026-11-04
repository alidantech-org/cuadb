//! Application authentication, sessions, permissions and audit.
//!
//! State lives in CuaDB's own SQLite file (`CUADB_DATA_DIR`), never in a target
//! database. Authentication is enforced once an administrator exists (or when
//! `CUADB_AUTH=required`); until then the local read-only shell stays open.
//! Every failure here is a closed enum: no driver text, hashes or secrets leak.

pub mod audit;
pub mod bootstrap;
pub mod password;
pub mod rbac;
pub mod service;
pub mod session;
pub mod store;

pub use rbac::{Action, Principal, Resource, Role};
pub use service::AuthService;
pub use store::AuthStore;

/// Secret-free errors for the auth layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    /// The state store could not be opened or queried.
    Storage,
    /// Input failed validation; the message is a fixed, safe sentence.
    Invalid(&'static str),
    /// A unique value (login) already exists.
    Exists,
    NotFound,
    /// Wrong credentials, disabled account, expired session: never says which.
    Denied,
    /// Too many failed attempts for now.
    Locked,
    /// Bootstrap refused because state exists or is ambiguous.
    BootstrapRefused(&'static str),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage => f.write_str("the authentication store is unavailable"),
            Self::Invalid(message) | Self::BootstrapRefused(message) => f.write_str(message),
            Self::Exists => f.write_str("that login already exists"),
            Self::NotFound => f.write_str("not found"),
            Self::Denied => f.write_str("authentication failed"),
            Self::Locked => f.write_str("too many attempts, try again later"),
        }
    }
}

impl std::error::Error for AuthError {}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Record that an export was served (relation identity only, never contents).
pub async fn audit_export(
    state: &crate::state::AppState,
    principal: &Principal,
    relation: &crate::db::types::RelationId,
    format: &str,
) {
    if let Some(auth) = state.auth().filter(|_| !principal.is_open()) {
        auth.record(
            audit::AuditEvent::new(audit::kind::EXPORT, audit::OK)
                .actor(principal.user_id(), principal.login())
                .resource(&format!("{}.{}", relation.namespace, relation.name))
                .detail(format),
        )
        .await;
    }
}

/// Record that a signed-in user opened a profile (catalog tree load).
pub async fn audit_profile_access(
    state: &crate::state::AppState,
    principal: &Principal,
    profile: &str,
) {
    if let Some(auth) = state.auth().filter(|_| !principal.is_open()) {
        auth.record(
            audit::AuditEvent::new(audit::kind::PROFILE_ACCESS, audit::OK)
                .actor(principal.user_id(), principal.login())
                .profile(profile),
        )
        .await;
    }
}
