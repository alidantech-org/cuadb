//! The auth service: login, sessions, user and grant administration, audit.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use super::{
    AuthError,
    audit::{self, AuditEvent, StoredEvent, kind},
    now, password,
    rbac::{Grant, Principal, PrincipalData, Role},
    session,
    store::{AuthStore, UserRow},
};

#[derive(Debug, Clone, Copy)]
pub struct SessionPolicy {
    pub idle_secs: i64,
    pub max_secs: i64,
    pub cookie_secure: bool,
}

const MAX_FAILURES: u32 = 5;
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
const LOCKOUT: Duration = Duration::from_secs(15 * 60);
const LIMITER_CAPACITY: usize = 10_000;

#[derive(Default)]
struct Attempts {
    failures: u32,
    window_start: Option<Instant>,
    locked_until: Option<Instant>,
}

/// A successful login: the cookie to set and the CSRF token for forms.
pub struct LoginOutcome {
    pub token: String,
    pub csrf: String,
    pub principal: Principal,
}

#[derive(Clone)]
pub struct AuthService {
    pub store: AuthStore,
    policy: SessionPolicy,
    enforced: Arc<AtomicBool>,
    limiter: Arc<Mutex<HashMap<String, Attempts>>>,
}

impl AuthService {
    pub async fn new(
        store: AuthStore,
        policy: SessionPolicy,
        force: bool,
    ) -> Result<Self, AuthError> {
        let service = Self {
            store,
            policy,
            enforced: Arc::new(AtomicBool::new(force)),
            limiter: Arc::new(Mutex::new(HashMap::new())),
        };
        service.refresh_enforced().await?;
        Ok(service)
    }

    pub fn policy(&self) -> SessionPolicy {
        self.policy
    }

    /// Auth is enforced as soon as any user exists (never silently relaxed afterwards).
    pub async fn refresh_enforced(&self) -> Result<(), AuthError> {
        if self.store.user_count().await? > 0 {
            self.enforced.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn is_enforced(&self) -> bool {
        self.enforced.load(Ordering::SeqCst)
    }

    pub async fn record(&self, event: AuditEvent) {
        // Auditing must never take the request down, but a failure is visible to the operator.
        if self.store.audit(&event).await.is_err() {
            eprintln!("level=error event=audit_write_failed kind={}", event.kind);
        }
    }

    fn locked(&self, key: &str) -> bool {
        let mut map = self.limiter.lock().unwrap_or_else(|p| p.into_inner());
        match map.get_mut(key) {
            Some(a) => match a.locked_until {
                Some(until) if until > Instant::now() => true,
                Some(_) => {
                    *a = Attempts::default();
                    false
                }
                None => false,
            },
            None => false,
        }
    }

    fn note_failure(&self, key: &str) -> bool {
        let mut map = self.limiter.lock().unwrap_or_else(|p| p.into_inner());
        if map.len() > LIMITER_CAPACITY {
            map.clear();
        }
        let a = map.entry(key.to_string()).or_default();
        let t = Instant::now();
        if a.window_start
            .is_none_or(|start| t.duration_since(start) > FAILURE_WINDOW)
        {
            a.window_start = Some(t);
            a.failures = 0;
        }
        a.failures += 1;
        if a.failures >= MAX_FAILURES {
            a.locked_until = Some(t + LOCKOUT);
            return true;
        }
        false
    }

    fn clear_failures(&self, key: &str) {
        self.limiter
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(key);
    }

    /// Verify credentials and open a session. Unknown login, wrong password and a
    /// disabled account are indistinguishable to the caller.
    pub async fn login(
        &self,
        raw_login: &str,
        password_input: &str,
    ) -> Result<LoginOutcome, AuthError> {
        let login = password::normalize_login(raw_login).map_err(|_| AuthError::Denied)?;
        if self.locked(&login) {
            self.record(AuditEvent::new(kind::LOGIN_LOCKED, audit::DENIED).actor(None, &login))
                .await;
            return Err(AuthError::Locked);
        }
        let user = self.store.user_by_login(&login).await?;
        let verified = match &user {
            Some(u) if u.active => password::verify(&u.password_hash, password_input),
            Some(_) | None => {
                password::burn(password_input);
                false
            }
        };
        let Some(user) = user.filter(|_| verified) else {
            let locked_now = self.note_failure(&login);
            self.record(
                AuditEvent::new(kind::LOGIN_FAILED, audit::FAILED)
                    .actor(None, &login)
                    .detail(if locked_now {
                        "locked"
                    } else {
                        "bad credentials"
                    }),
            )
            .await;
            return Err(AuthError::Denied);
        };
        self.clear_failures(&login);
        let token = session::new_token();
        let csrf = session::new_token();
        let expires = now() + self.policy.max_secs;
        self.store
            .create_session(&session::hash_token(&token), user.id, &csrf, expires)
            .await?;
        self.record(AuditEvent::new(kind::LOGIN_OK, audit::OK).actor(Some(user.id), &user.login))
            .await;
        let principal = self
            .principal_for(&user, &session::hash_token(&token), csrf.clone())
            .await?;
        Ok(LoginOutcome {
            token,
            csrf,
            principal,
        })
    }

    async fn principal_for(
        &self,
        user: &UserRow,
        session_hash: &[u8],
        csrf: String,
    ) -> Result<Principal, AuthError> {
        let grants = self.store.grants_for_user(user.id).await?;
        Ok(Principal(Arc::new(PrincipalData {
            user_id: Some(user.id),
            login: user.login.clone(),
            role: Role::parse(&user.role).map_err(|_| AuthError::Storage)?,
            csrf,
            session_hash: session_hash.to_vec(),
            grants,
            open: false,
        })))
    }

    /// Resolve a cookie token to a principal, enforcing idle and absolute expiry.
    pub async fn authenticate(&self, token: &str) -> Option<Principal> {
        if !session::valid_token_shape(token) {
            return None;
        }
        let hash = session::hash_token(token);
        let row = self.store.session(&hash).await.ok()??;
        let t = now();
        if row.expires_at <= t || t - row.last_seen > self.policy.idle_secs {
            let _ = self.store.revoke_session(&hash).await;
            return None;
        }
        let user = self.store.user_by_id(row.user_id).await.ok()??;
        if !user.active {
            let _ = self.store.revoke_session(&hash).await;
            return None;
        }
        let _ = self.store.touch_session(&hash).await;
        self.principal_for(&user, &hash, row.csrf).await.ok()
    }

    /// Is this stored session still valid (not revoked, not expired, user active)?
    pub async fn session_alive(&self, hash: &[u8]) -> bool {
        let Ok(Some(row)) = self.store.session(hash).await else {
            return false;
        };
        let t = now();
        if row.expires_at <= t || t - row.last_seen > self.policy.idle_secs {
            return false;
        }
        matches!(self.store.user_by_id(row.user_id).await, Ok(Some(u)) if u.active)
    }

    pub async fn logout(&self, principal: &Principal) {
        let _ = self.store.revoke_session(&principal.0.session_hash).await;
        self.record(
            AuditEvent::new(kind::LOGOUT, audit::OK).actor(principal.user_id(), principal.login()),
        )
        .await;
    }

    // ---- administration (callers must have checked `principal.is_admin()`)

    pub async fn create_user(
        &self,
        admin: &Principal,
        raw_login: &str,
        password_input: &str,
        role: Role,
    ) -> Result<i64, AuthError> {
        let login = password::normalize_login(raw_login)?;
        password::validate_password(&login, password_input)?;
        let hash = password::hash(password_input)?;
        let id = self.store.create_user(&login, &hash, role.name()).await?;
        self.record(
            AuditEvent::new(kind::USER_CREATED, audit::OK)
                .actor(admin.user_id(), admin.login())
                .resource(&login)
                .detail(role.name()),
        )
        .await;
        self.refresh_enforced().await?;
        Ok(id)
    }

    /// Disable or enable a user; disabling revokes every session. The last active
    /// administrator can never be disabled.
    pub async fn set_user_active(
        &self,
        admin: &Principal,
        user_id: i64,
        active: bool,
    ) -> Result<(), AuthError> {
        let user = self
            .store
            .user_by_id(user_id)
            .await?
            .ok_or(AuthError::NotFound)?;
        if !active
            && user.role == "admin"
            && user.active
            && self.store.active_admin_count().await? <= 1
        {
            return Err(AuthError::Invalid(
                "the last active administrator cannot be disabled",
            ));
        }
        self.store.set_user_active(user_id, active).await?;
        if !active {
            self.store.revoke_user_sessions(user_id).await?;
            self.record(
                AuditEvent::new(kind::SESSION_REVOKED, audit::OK)
                    .actor(admin.user_id(), admin.login())
                    .resource(&user.login),
            )
            .await;
        }
        self.record(
            AuditEvent::new(kind::USER_STATUS, audit::OK)
                .actor(admin.user_id(), admin.login())
                .resource(&user.login)
                .detail(if active { "enabled" } else { "disabled" }),
        )
        .await;
        Ok(())
    }

    /// Change a password (own account needs the current password; admins reset others).
    pub async fn change_password(
        &self,
        actor: &Principal,
        user_id: i64,
        current: Option<&str>,
        new_password: &str,
    ) -> Result<(), AuthError> {
        let user = self
            .store
            .user_by_id(user_id)
            .await?
            .ok_or(AuthError::NotFound)?;
        let own = actor.user_id() == Some(user_id);
        if !own && !actor.is_admin() {
            return Err(AuthError::Denied);
        }
        if own && !actor.is_admin() || own && current.is_some() {
            let ok = current.is_some_and(|c| password::verify(&user.password_hash, c));
            if !ok {
                return Err(AuthError::Denied);
            }
        }
        password::validate_password(&user.login, new_password)?;
        let hash = password::hash(new_password)?;
        self.store.set_password(user_id, &hash).await?;
        // A password change ends every other session of that user.
        let keep = actor.0.session_hash.clone();
        if own {
            self.revoke_others(user_id, &keep).await?;
        } else {
            self.store.revoke_user_sessions(user_id).await?;
        }
        self.record(
            AuditEvent::new(kind::PASSWORD_CHANGED, audit::OK)
                .actor(actor.user_id(), actor.login())
                .resource(&user.login),
        )
        .await;
        Ok(())
    }

    async fn revoke_others(&self, user_id: i64, keep: &[u8]) -> Result<(), AuthError> {
        sqlx::query("UPDATE sessions SET revoked = 1 WHERE user_id = ? AND id_hash != ?")
            .bind(user_id)
            .bind(keep)
            .execute(&self.store.pool)
            .await
            .map(|_| ())
            .map_err(|_| AuthError::Storage)
    }

    pub async fn add_grant(
        &self,
        admin: &Principal,
        user_id: i64,
        profile: &str,
        schema: &str,
        relation: &str,
        actions: u8,
    ) -> Result<i64, AuthError> {
        if !crate::profiles::valid_id(profile) {
            return Err(AuthError::Invalid("invalid profile id"));
        }
        for part in [schema, relation] {
            if part.len() > 128 || part.chars().any(char::is_control) {
                return Err(AuthError::Invalid("invalid schema or relation"));
            }
        }
        if schema.is_empty() && !relation.is_empty() {
            return Err(AuthError::Invalid("a relation grant needs its schema"));
        }
        let id = self
            .store
            .add_grant(user_id, profile, schema, relation, actions)
            .await?;
        self.record(
            AuditEvent::new(kind::GRANT_ADDED, audit::OK)
                .actor(admin.user_id(), admin.login())
                .profile(profile)
                .resource(&format!("user#{user_id} {schema}.{relation}"))
                .detail(&super::rbac::action_names(actions)),
        )
        .await;
        Ok(id)
    }

    pub async fn remove_grant(&self, admin: &Principal, grant_id: i64) -> Result<(), AuthError> {
        self.store.remove_grant(grant_id).await?;
        self.record(
            AuditEvent::new(kind::GRANT_REMOVED, audit::OK)
                .actor(admin.user_id(), admin.login())
                .resource(&format!("grant#{grant_id}")),
        )
        .await;
        Ok(())
    }

    pub async fn grants(&self, user_id: i64) -> Result<Vec<Grant>, AuthError> {
        self.store.grants_for_user(user_id).await
    }

    pub async fn audit_events(
        &self,
        limit: i64,
        before: Option<i64>,
    ) -> Result<Vec<StoredEvent>, AuthError> {
        self.store.audit_events(limit, before).await
    }

    pub async fn audit_events_filtered(
        &self,
        filter: &crate::auth::store::AuditFilter,
        limit: i64,
        before: Option<i64>,
    ) -> Result<Vec<StoredEvent>, AuthError> {
        self.store
            .audit_events_filtered(filter, limit, before)
            .await
    }

    /// Change a member's role. Nobody changes their own role, and the last active
    /// administrator cannot be demoted.
    pub async fn set_user_role(
        &self,
        admin: &Principal,
        user_id: i64,
        role: Role,
    ) -> Result<(), AuthError> {
        if admin.user_id() == Some(user_id) {
            return Err(AuthError::Invalid("you cannot change your own role"));
        }
        let user = self
            .store
            .user_by_id(user_id)
            .await?
            .ok_or(AuthError::NotFound)?;
        if user.role == "admin"
            && role != Role::Admin
            && user.active
            && self.store.active_admin_count().await? <= 1
        {
            return Err(AuthError::Invalid(
                "the last active administrator cannot be demoted",
            ));
        }
        self.store.set_user_role(user_id, role.name()).await?;
        self.record(
            AuditEvent::new(kind::USER_ROLE, audit::OK)
                .actor(admin.user_id(), admin.login())
                .resource(&user.login)
                .detail(role.name()),
        )
        .await;
        Ok(())
    }

    /// Delete a member (sessions and grants go too). Never yourself, never the last
    /// active administrator.
    pub async fn delete_user(&self, admin: &Principal, user_id: i64) -> Result<(), AuthError> {
        if admin.user_id() == Some(user_id) {
            return Err(AuthError::Invalid("you cannot delete your own account"));
        }
        let user = self
            .store
            .user_by_id(user_id)
            .await?
            .ok_or(AuthError::NotFound)?;
        if user.role == "admin" && user.active && self.store.active_admin_count().await? <= 1 {
            return Err(AuthError::Invalid(
                "the last active administrator cannot be deleted",
            ));
        }
        self.store.delete_user(user_id).await?;
        self.record(
            AuditEvent::new(kind::USER_DELETED, audit::OK)
                .actor(admin.user_id(), admin.login())
                .resource(&user.login),
        )
        .await;
        Ok(())
    }
}
