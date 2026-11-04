//! Argon2id password hashing with per-password salts, plus login/password rules.

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use super::AuthError;

pub const MIN_PASSWORD_CHARS: usize = 12;
pub const MAX_PASSWORD_BYTES: usize = 256;

fn argon2() -> Argon2<'static> {
    // OWASP minimum for Argon2id: 19 MiB, 2 iterations, 1 lane.
    let params = Params::new(19_456, 2, 1, None).expect("static argon2 parameters are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Lower-cased, trimmed login of 3-64 characters from `a-z 0-9 . _ @ -`.
pub fn normalize_login(raw: &str) -> Result<String, AuthError> {
    let login = raw.trim().to_lowercase();
    let ok = (3..=64).contains(&login.chars().count())
        && login.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '@' | '-')
        })
        && login.starts_with(|c: char| c.is_ascii_alphanumeric());
    if ok {
        Ok(login)
    } else {
        Err(AuthError::Invalid(
            "login must be 3-64 characters of a-z, 0-9, '.', '_', '@' or '-'",
        ))
    }
}

pub fn validate_password(login: &str, password: &str) -> Result<(), AuthError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(AuthError::Invalid(
            "password must be at least 12 characters",
        ));
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(AuthError::Invalid("password is too long"));
    }
    if password.chars().any(char::is_control) {
        return Err(AuthError::Invalid(
            "password must not contain control characters",
        ));
    }
    let lowered = password.to_lowercase();
    if lowered.contains(&login.to_lowercase()) {
        return Err(AuthError::Invalid("password must not contain the login"));
    }
    let first = password.chars().next();
    if password.chars().all(|c| Some(c) == first) {
        return Err(AuthError::Invalid("password is too simple"));
    }
    Ok(())
}

pub fn hash(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::Storage)
}

pub fn verify(stored: &str, password: &str) -> bool {
    match PasswordHash::new(stored) {
        Ok(parsed) => argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Spend the same effort when the login does not exist, so response time does not
/// reveal which logins are valid.
pub fn burn(password: &str) {
    use std::sync::OnceLock;
    static DUMMY: OnceLock<String> = OnceLock::new();
    let dummy = DUMMY.get_or_init(|| hash("dummy-password-for-timing").unwrap_or_default());
    let _ = verify(dummy, password);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_salted_argon2id_and_verify() {
        let a = hash("correct horse battery").unwrap();
        let b = hash("correct horse battery").unwrap();
        assert_ne!(a, b, "per-password salt");
        assert!(a.starts_with("$argon2id$"));
        assert!(verify(&a, "correct horse battery"));
        assert!(!verify(&a, "correct horse batterY"));
        assert!(!verify("not-a-hash", "x"));
        assert!(!a.contains("correct"));
    }

    #[test]
    fn logins_normalize_and_reject_bad_shapes() {
        assert_eq!(
            normalize_login("  Admin.User@Example ").unwrap(),
            "admin.user@example"
        );
        for bad in [
            "ab",
            "",
            "a b",
            "-lead",
            "x".repeat(65).as_str(),
            "ünï",
            "a/b",
            "a\u{0}b",
        ] {
            assert!(normalize_login(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn password_policy_rejects_weak_values() {
        assert!(validate_password("alice", "short").is_err());
        assert!(validate_password("alice", "xxxxxxxxxxxxxx").is_err());
        assert!(validate_password("alice", "has-alice-in-it!!").is_err());
        assert!(validate_password("alice", "tab\there-12345678").is_err());
        assert!(validate_password("alice", &"a1".repeat(200)).is_err());
        assert!(validate_password("alice", "a long enough passphrase").is_ok());
    }
}
