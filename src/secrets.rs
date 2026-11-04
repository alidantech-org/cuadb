//! Encryption of saved connection secrets (AES-256-GCM).
//!
//! The key comes from the operator (`CUADB_SECRET_KEY`, 64 hex characters, e.g. from
//! `openssl rand -hex 32`). It is never stored, logged or shown. Each value is sealed
//! with a fresh random nonce and bound to its connection id, so a ciphertext cannot be
//! moved to another record.

use std::fmt;

use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};

/// A validated 256-bit key. `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey([u8; 32]);

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey([REDACTED])")
    }
}

impl SecretKey {
    /// Exactly 64 hexadecimal characters.
    pub fn parse_hex(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut key = [0u8; 32];
        for (index, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
        }
        Some(Self(key))
    }
}

#[derive(Clone)]
pub struct SecretBox {
    key: std::sync::Arc<LessSafeKey>,
}

impl fmt::Debug for SecretBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretBox([REDACTED])")
    }
}

impl SecretBox {
    pub fn new(key: &SecretKey) -> Self {
        // A 32-byte key is always valid for AES-256-GCM.
        let unbound = UnboundKey::new(&AES_256_GCM, &key.0).expect("32-byte AES-256 key");
        Self {
            key: std::sync::Arc::new(LessSafeKey::new(unbound)),
        }
    }

    /// `nonce || ciphertext || tag`.
    pub fn seal(&self, aad: &str, plain: &[u8]) -> Option<Vec<u8>> {
        let mut nonce = [0u8; NONCE_LEN];
        SystemRandom::new().fill(&mut nonce).ok()?;
        let mut buffer = plain.to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad.as_bytes()),
                &mut buffer,
            )
            .ok()?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&buffer);
        Some(out)
    }

    /// `None` for a wrong key, a changed record id, or tampered data.
    pub fn open(&self, aad: &str, blob: &[u8]) -> Option<Vec<u8>> {
        if blob.len() < NONCE_LEN + 16 {
            return None;
        }
        let (nonce, rest) = blob.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).ok()?;
        let mut buffer = rest.to_vec();
        let plain = self
            .key
            .open_in_place(nonce, Aad::from(aad.as_bytes()), &mut buffer)
            .ok()?;
        Some(plain.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: char) -> SecretKey {
        SecretKey::parse_hex(&seed.to_string().repeat(64)).unwrap()
    }

    #[test]
    fn round_trips_and_binds_to_the_record_id() {
        let sealed = SecretBox::new(&key('a'));
        let blob = sealed.seal("pg-prod", b"postgresql://u:pw@h/db").unwrap();
        assert!(!blob.windows(2).any(|w| w == b"pw"), "plaintext leaked");
        assert_eq!(
            sealed.open("pg-prod", &blob).unwrap(),
            b"postgresql://u:pw@h/db"
        );
        assert!(sealed.open("other", &blob).is_none(), "id is authenticated");
        assert!(SecretBox::new(&key('b')).open("pg-prod", &blob).is_none());
        let mut tampered = blob.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(sealed.open("pg-prod", &tampered).is_none());
        assert_ne!(
            blob,
            sealed.seal("pg-prod", b"postgresql://u:pw@h/db").unwrap(),
            "fresh nonce"
        );
    }

    #[test]
    fn keys_must_be_64_hex_characters_and_never_print() {
        assert!(SecretKey::parse_hex("abc").is_none());
        assert!(SecretKey::parse_hex(&"g".repeat(64)).is_none());
        assert_eq!(format!("{:?}", key('c')), "SecretKey([REDACTED])");
    }
}
