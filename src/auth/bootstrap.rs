//! First-run administrator bootstrap and local recovery, driven by the operator
//! through `cuadb admin ...` (never through an HTTP endpoint).

use super::{
    AuthError,
    audit::{self, AuditEvent, kind},
    password,
    store::AuthStore,
};

/// Create the very first administrator. Fails closed if any user exists or the
/// bootstrap marker is already set (for example after users were removed by hand).
pub async fn bootstrap_admin(
    store: &AuthStore,
    login: &str,
    password_input: &str,
) -> Result<(), AuthError> {
    let login = password::normalize_login(login)?;
    password::validate_password(&login, password_input)?;
    // Check first so a refused attempt does not even spend hashing time.
    if store.user_count().await? > 0 || store.meta("bootstrapped_at").await?.is_some() {
        return Err(AuthError::BootstrapRefused(
            "setup already happened or state is ambiguous",
        ));
    }
    let hash = password::hash(password_input)?;
    let id = store.bootstrap_admin(&login, &hash).await?;
    let _ = store
        .audit(
            &AuditEvent::new(kind::BOOTSTRAP, audit::OK)
                .actor(Some(id), &login)
                .detail("first administrator created"),
        )
        .await;
    Ok(())
}

/// Operator recovery with filesystem access: reset a password and re-enable the account.
pub async fn reset_password(
    store: &AuthStore,
    login: &str,
    password_input: &str,
) -> Result<(), AuthError> {
    let login = password::normalize_login(login)?;
    password::validate_password(&login, password_input)?;
    let user = store
        .user_by_login(&login)
        .await?
        .ok_or(AuthError::NotFound)?;
    store
        .set_password(user.id, &password::hash(password_input)?)
        .await?;
    store.set_user_active(user.id, true).await?;
    store.revoke_user_sessions(user.id).await?;
    let _ = store
        .audit(
            &AuditEvent::new(kind::PASSWORD_CHANGED, audit::OK)
                .actor(None, "operator-cli")
                .resource(&login),
        )
        .await;
    Ok(())
}
