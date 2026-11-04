//! Append-only audit events. Never store passwords, URLs, query values or row data.

/// Kinds are a closed vocabulary so a log reader can rely on them.
pub mod kind {
    pub const LOGIN_OK: &str = "login_ok";
    pub const LOGIN_FAILED: &str = "login_failed";
    pub const LOGIN_LOCKED: &str = "login_locked";
    pub const LOGOUT: &str = "logout";
    pub const SESSION_REVOKED: &str = "session_revoked";
    pub const USER_CREATED: &str = "user_created";
    pub const USER_STATUS: &str = "user_status_changed";
    pub const PASSWORD_CHANGED: &str = "password_changed";
    pub const GRANT_ADDED: &str = "grant_added";
    pub const GRANT_REMOVED: &str = "grant_removed";
    pub const PROFILE_ACCESS: &str = "profile_access";
    pub const EXPORT: &str = "export";
    pub const ACCESS_DENIED: &str = "access_denied";
    pub const BOOTSTRAP: &str = "bootstrap";
    pub const SQL_EXECUTE: &str = "sql_execute";
    pub const DATA_CHANGE: &str = "data_change";
    pub const TX_BEGIN: &str = "tx_begin";
    pub const TX_COMMIT: &str = "tx_commit";
    pub const TX_ROLLBACK: &str = "tx_rollback";
}

pub const OK: &str = "ok";
pub const DENIED: &str = "denied";
pub const FAILED: &str = "failed";

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub kind: &'static str,
    pub actor_id: Option<i64>,
    pub actor: String,
    pub profile: Option<String>,
    pub resource: Option<String>,
    pub outcome: &'static str,
    pub detail: Option<String>,
}

impl AuditEvent {
    pub fn new(kind: &'static str, outcome: &'static str) -> Self {
        Self {
            kind,
            actor_id: None,
            actor: "-".into(),
            profile: None,
            resource: None,
            outcome,
            detail: None,
        }
    }

    pub fn actor(mut self, id: Option<i64>, login: &str) -> Self {
        self.actor_id = id;
        self.actor = clean(login, 64);
        self
    }

    pub fn profile(mut self, profile: &str) -> Self {
        self.profile = Some(clean(profile, 32));
        self
    }

    pub fn resource(mut self, resource: &str) -> Self {
        self.resource = Some(clean(resource, 200));
        self
    }

    pub fn detail(mut self, detail: &str) -> Self {
        self.detail = Some(clean(detail, 200));
        self
    }
}

/// Bound length and drop control characters so a hostile value cannot forge log lines.
pub fn clean(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub id: i64,
    pub ts: i64,
    pub actor: String,
    pub kind: String,
    pub profile: String,
    pub resource: String,
    pub outcome: String,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_bounded_and_stripped_of_control_characters() {
        let event = AuditEvent::new(kind::LOGIN_FAILED, FAILED)
            .actor(None, "ali\nce\u{0}")
            .resource(&"x".repeat(1000))
            .detail("line1\r\nforged: ok");
        assert_eq!(event.actor, "alice");
        assert_eq!(event.resource.as_deref().unwrap().len(), 200);
        assert!(!event.detail.unwrap().contains('\n'));
    }
}
