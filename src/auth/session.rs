//! Opaque session tokens and cookie handling. Only a SHA-256 of the token is stored.

use sha2::{Digest, Sha256};

pub const COOKIE_NAME: &str = "cuadb_session";

/// 256 random bits as 64 hex characters.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("operating system randomness is available");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn valid_token_shape(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn set_cookie(token: &str, max_age_secs: i64, secure: bool) -> String {
    format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age_secs}{}",
        if secure { "; Secure" } else { "" }
    )
}

pub fn clear_cookie(secure: bool) -> String {
    format!(
        "{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}",
        if secure { "; Secure" } else { "" }
    )
}

/// The token from a `Cookie` header, if it has the exact shape.
pub fn token_from_cookie_header(header: &str) -> Option<&str> {
    header
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value)
        .filter(|value| valid_token_shape(value))
}

/// Constant-time equality for equal-length secrets (CSRF tokens).
pub fn secrets_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_hex_and_hashed_for_storage() {
        let a = new_token();
        let b = new_token();
        assert_ne!(a, b);
        assert!(valid_token_shape(&a));
        assert_eq!(hash_token(&a).len(), 32);
        assert_ne!(hash_token(&a), a.as_bytes());
    }

    #[test]
    fn cookie_flags_and_parsing() {
        let token = new_token();
        let cookie = set_cookie(&token, 60, true);
        for flag in [
            "HttpOnly",
            "SameSite=Strict",
            "Path=/",
            "Max-Age=60",
            "Secure",
        ] {
            assert!(cookie.contains(flag), "{flag}");
        }
        assert!(!set_cookie(&token, 60, false).contains("Secure"));
        assert!(clear_cookie(false).contains("Max-Age=0"));
        let header = format!("theme=dark; {COOKIE_NAME}={token}; other=1");
        assert_eq!(token_from_cookie_header(&header), Some(token.as_str()));
        assert_eq!(
            token_from_cookie_header(&format!("{COOKIE_NAME}=short")),
            None
        );
        assert_eq!(
            token_from_cookie_header(&format!("{COOKIE_NAME}={}", "z".repeat(64))),
            None
        );
    }

    #[test]
    fn secret_comparison_is_exact() {
        assert!(secrets_equal("abc", "abc"));
        assert!(!secrets_equal("abc", "abd"));
        assert!(!secrets_equal("abc", "abcd"));
    }
}
