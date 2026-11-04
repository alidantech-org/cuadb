//! Auth core against an in-memory store: bootstrap, users, login, sessions, audit.

use cuadb::auth::{
    AuthError, AuthService, Role,
    bootstrap::{bootstrap_admin, reset_password},
    service::SessionPolicy,
    store::AuthStore,
};

const PW: &str = "a long enough passphrase";

async fn service() -> AuthService {
    let store = AuthStore::open_memory().await.unwrap();
    AuthService::new(
        store,
        SessionPolicy {
            idle_secs: 1800,
            max_secs: 43_200,
            cookie_secure: false,
        },
        false,
    )
    .await
    .unwrap()
}

async fn with_admin() -> (AuthService, cuadb::auth::Principal) {
    let svc = service().await;
    bootstrap_admin(&svc.store, "Admin", PW).await.unwrap();
    svc.refresh_enforced().await.unwrap();
    let login = svc.login("admin", PW).await.unwrap();
    (svc, login.principal)
}

#[tokio::test]
async fn bootstrap_is_one_time_validated_and_enables_enforcement() {
    let svc = service().await;
    assert!(!svc.is_enforced(), "no users: local shell stays open");
    assert!(matches!(
        bootstrap_admin(&svc.store, "ab", PW).await,
        Err(AuthError::Invalid(_))
    ));
    assert!(matches!(
        bootstrap_admin(&svc.store, "admin", "short").await,
        Err(AuthError::Invalid(_))
    ));
    assert_eq!(
        svc.store.user_count().await.unwrap(),
        0,
        "invalid attempts create nothing"
    );
    bootstrap_admin(&svc.store, "admin", PW).await.unwrap();
    svc.refresh_enforced().await.unwrap();
    assert!(svc.is_enforced());
    // Repeated setup is refused, with the same answer for any input.
    for login in ["admin", "other-admin"] {
        assert!(matches!(
            bootstrap_admin(&svc.store, login, "another passphrase!").await,
            Err(AuthError::BootstrapRefused(_))
        ));
    }
    assert_eq!(svc.store.user_count().await.unwrap(), 1);
    let stored = svc.store.user_by_login("admin").await.unwrap().unwrap();
    assert!(stored.password_hash.starts_with("$argon2id$") && !stored.password_hash.contains(PW));
    assert_eq!(stored.role, "admin");
}

#[tokio::test]
async fn ambiguous_state_fails_closed() {
    let svc = service().await;
    bootstrap_admin(&svc.store, "admin", PW).await.unwrap();
    // Someone removed the users but the marker stays: setup must not run again silently.
    sqlx::query("DELETE FROM users")
        .execute(&svc.store.pool)
        .await
        .unwrap();
    assert!(matches!(
        bootstrap_admin(&svc.store, "admin", PW).await,
        Err(AuthError::BootstrapRefused(_))
    ));
}

#[tokio::test]
async fn interrupted_bootstrap_leaves_nothing_behind() {
    let svc = service().await;
    // A transaction that is dropped before commit must not create the admin or the marker.
    {
        let mut tx = svc.store.pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO users (login, password_hash, role, created_at) VALUES ('x','h','admin',0)",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    assert_eq!(svc.store.user_count().await.unwrap(), 0);
    assert!(svc.store.meta("bootstrapped_at").await.unwrap().is_none());
    bootstrap_admin(&svc.store, "admin", PW).await.unwrap();
}

#[tokio::test]
async fn login_verifies_passwords_and_hides_why_it_failed() {
    let (svc, admin) = with_admin().await;
    svc.create_user(&admin, "carol", "correct horse battery 1", Role::Viewer)
        .await
        .unwrap();
    assert!(svc.login("carol", "correct horse battery 1").await.is_ok());
    for (login, password) in [
        ("carol", "wrong password 123"),
        ("nobody", PW),
        ("", PW),
        ("carol", ""),
    ] {
        assert!(
            matches!(svc.login(login, password).await, Err(AuthError::Denied)),
            "{login}"
        );
    }
    // Disabled users look exactly like wrong passwords.
    let carol = svc.store.user_by_login("carol").await.unwrap().unwrap();
    svc.set_user_active(&admin, carol.id, false).await.unwrap();
    assert!(matches!(
        svc.login("carol", "correct horse battery 1").await,
        Err(AuthError::Denied)
    ));
    svc.set_user_active(&admin, carol.id, true).await.unwrap();
    assert!(svc.login("carol", "correct horse battery 1").await.is_ok());
}

#[tokio::test]
async fn users_are_unique_and_the_last_admin_cannot_be_disabled() {
    let (svc, admin) = with_admin().await;
    assert!(
        matches!(
            svc.create_user(&admin, "ADMIN", PW, Role::Viewer).await,
            Err(AuthError::Exists)
        ),
        "logins are case-insensitive"
    );
    assert!(matches!(
        svc.create_user(&admin, "bob", "short", Role::Viewer).await,
        Err(AuthError::Invalid(_))
    ));
    assert!(matches!(
        svc.create_user(&admin, "bob!", PW, Role::Viewer).await,
        Err(AuthError::Invalid(_))
    ));
    let me = svc.store.user_by_login("admin").await.unwrap().unwrap();
    assert!(matches!(
        svc.set_user_active(&admin, me.id, false).await,
        Err(AuthError::Invalid(_))
    ));
    assert!(matches!(
        svc.set_user_active(&admin, 9999, false).await,
        Err(AuthError::NotFound)
    ));
}

#[tokio::test]
async fn sessions_expire_by_idle_time_and_absolute_age_and_can_be_revoked() {
    let (svc, _) = with_admin().await;
    let login = svc.login("admin", PW).await.unwrap();
    assert!(svc.authenticate(&login.token).await.is_some());
    assert!(svc.authenticate("not-a-token").await.is_none());
    assert!(svc.authenticate(&"0".repeat(64)).await.is_none());
    // Idle expiry.
    sqlx::query("UPDATE sessions SET last_seen = last_seen - 100000")
        .execute(&svc.store.pool)
        .await
        .unwrap();
    assert!(
        svc.authenticate(&login.token).await.is_none(),
        "idle session expires"
    );
    // Absolute expiry.
    let fresh = svc.login("admin", PW).await.unwrap();
    sqlx::query("UPDATE sessions SET expires_at = 1")
        .execute(&svc.store.pool)
        .await
        .unwrap();
    assert!(
        svc.authenticate(&fresh.token).await.is_none(),
        "absolute expiry"
    );
    // Logout revokes.
    let third = svc.login("admin", PW).await.unwrap();
    svc.logout(&third.principal).await;
    assert!(svc.authenticate(&third.token).await.is_none());
}

#[tokio::test]
async fn disabling_a_user_revokes_their_sessions_and_password_changes_end_other_sessions() {
    let (svc, admin) = with_admin().await;
    svc.create_user(&admin, "dave", "horse battery staple 22", Role::Editor)
        .await
        .unwrap();
    let first = svc.login("dave", "horse battery staple 22").await.unwrap();
    let second = svc.login("dave", "horse battery staple 22").await.unwrap();
    // Own password change needs the current password, keeps this session, ends the other.
    let dave = second.principal.clone();
    assert!(matches!(
        svc.change_password(
            &dave,
            dave.user_id().unwrap(),
            Some("wrong guess here"),
            "new long password 1!"
        )
        .await,
        Err(AuthError::Denied)
    ));
    svc.change_password(
        &dave,
        dave.user_id().unwrap(),
        Some("horse battery staple 22"),
        "new long password 1!",
    )
    .await
    .unwrap();
    assert!(svc.authenticate(&second.token).await.is_some());
    assert!(svc.authenticate(&first.token).await.is_none());
    assert!(svc.login("dave", "horse battery staple 22").await.is_err());
    assert!(svc.login("dave", "new long password 1!").await.is_ok());
    // A non-admin cannot reset somebody else's password.
    let me = svc.store.user_by_login("admin").await.unwrap().unwrap();
    assert!(matches!(
        svc.change_password(&dave, me.id, None, "hijacked password 123!")
            .await,
        Err(AuthError::Denied)
    ));
    // Disabling revokes live sessions.
    let live = svc.login("dave", "new long password 1!").await.unwrap();
    svc.set_user_active(&admin, dave.user_id().unwrap(), false)
        .await
        .unwrap();
    assert!(svc.authenticate(&live.token).await.is_none());
}

#[tokio::test]
async fn repeated_failures_lock_the_login_without_revealing_whether_it_exists() {
    let (svc, _) = with_admin().await;
    for _ in 0..5 {
        let _ = svc.login("admin", "bad guess number one").await;
    }
    assert!(
        matches!(svc.login("admin", PW).await, Err(AuthError::Locked)),
        "even the right password"
    );
    for _ in 0..5 {
        let _ = svc.login("ghost-user", "bad guess number one").await;
    }
    assert!(
        matches!(svc.login("ghost-user", PW).await, Err(AuthError::Locked)),
        "unknown logins lock the same way"
    );
}

#[tokio::test]
async fn operator_recovery_resets_the_password_and_reenables_the_account() {
    let (svc, admin) = with_admin().await;
    let me = svc.store.user_by_login("admin").await.unwrap().unwrap();
    let live = svc.login("admin", PW).await.unwrap();
    reset_password(&svc.store, "admin", "recovered passphrase 99")
        .await
        .unwrap();
    assert!(
        svc.authenticate(&live.token).await.is_none(),
        "recovery revokes sessions"
    );
    assert!(svc.login("admin", PW).await.is_err());
    assert!(svc.login("admin", "recovered passphrase 99").await.is_ok());
    assert!(matches!(
        reset_password(&svc.store, "ghost", PW).await,
        Err(AuthError::NotFound)
    ));
    let _ = (admin, me);
}
