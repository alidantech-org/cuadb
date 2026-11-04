//! Trusted, server-side connection profiles.
//!
//! Profiles are configured by the operator (a JSON file named by
//! `CUADB_PROFILES_FILE`, or the single `CUADB_DATABASE_URL`, which becomes the
//! profile `default`). The browser can only ever name a profile by its stable
//! `id`; it can never supply a URL, host or credential. Error values never echo
//! URLs, and `Debug` output redacts them.

use std::{collections::HashSet, fmt};

use crate::config::SecretUrl;

pub const DEFAULT_PROFILE_ID: &str = "default";
pub const MAX_PROFILES: usize = 8;
const MAX_FILE_BYTES: usize = 65_536;
const MAX_LABEL_CHARS: usize = 48;
const MAX_URL_BYTES: usize = 8192;

/// A validated profile. The URL is only reachable through `SecretUrl::expose`.
#[derive(Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub url: SecretUrl,
}

impl fmt::Debug for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Profile")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("url", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSet {
    pub profiles: Vec<Profile>,
    /// Index of the profile used when a request names none.
    pub default: usize,
}

/// A secret-free configuration error: it names a profile position, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileError(String);

impl ProfileError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid connection profiles: {}", self.0)
    }
}

impl std::error::Error for ProfileError {}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDocument {
    default: Option<String>,
    profiles: Vec<FileProfile>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileProfile {
    id: String,
    label: String,
    url: String,
}

/// Stable, URL-safe identifier: lowercase letters, digits, `-` and `_`, 1–32 bytes.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && !id.starts_with(['-', '_'])
}

fn valid_label(label: &str) -> bool {
    let trimmed = label.trim();
    !trimmed.is_empty()
        && trimmed == label
        && label.chars().count() <= MAX_LABEL_CHARS
        && !label.chars().any(char::is_control)
}

impl ProfileSet {
    /// The classic single-URL configuration.
    pub fn single(url: SecretUrl) -> Self {
        Self {
            profiles: vec![Profile {
                id: DEFAULT_PROFILE_ID.into(),
                label: "Default connection".into(),
                url,
            }],
            default: 0,
        }
    }

    pub fn from_json(text: &str) -> Result<Self, ProfileError> {
        if text.len() > MAX_FILE_BYTES {
            return Err(ProfileError::new("file is larger than 64 KiB"));
        }
        let document: FileDocument = serde_json::from_str(text)
            .map_err(|_| ProfileError::new("not valid JSON of the documented shape"))?;
        if document.profiles.is_empty() || document.profiles.len() > MAX_PROFILES {
            return Err(ProfileError::new("between 1 and 8 profiles are required"));
        }
        let mut ids = HashSet::new();
        let mut labels = HashSet::new();
        let mut profiles = Vec::new();
        for (index, raw) in document.profiles.into_iter().enumerate() {
            let position = index + 1;
            if !valid_id(&raw.id) {
                return Err(ProfileError::new(format!(
                    "profile {position}: id must be 1-32 chars of a-z, 0-9, '-' or '_'"
                )));
            }
            if !ids.insert(raw.id.clone()) {
                return Err(ProfileError::new(format!(
                    "profile {position}: duplicate id"
                )));
            }
            if !valid_label(&raw.label) {
                return Err(ProfileError::new(format!(
                    "profile {position}: label must be 1-48 printable characters without outer spaces"
                )));
            }
            if !labels.insert(raw.label.to_lowercase()) {
                return Err(ProfileError::new(format!(
                    "profile {position}: duplicate label"
                )));
            }
            if raw.url.is_empty() || raw.url.len() > MAX_URL_BYTES || raw.url.trim() != raw.url {
                return Err(ProfileError::new(format!(
                    "profile {position}: url must be nonempty, at most 8192 bytes, no outer whitespace"
                )));
            }
            profiles.push(Profile {
                id: raw.id,
                label: raw.label,
                url: SecretUrl::new(raw.url),
            });
        }
        let default = match document.default {
            None => 0,
            Some(id) => profiles
                .iter()
                .position(|p| p.id == id)
                .ok_or_else(|| ProfileError::new("default does not name a configured profile"))?,
        };
        Ok(Self { profiles, default })
    }

    pub fn find(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(profiles: &str) -> String {
        format!(r#"{{"profiles":[{profiles}]}}"#)
    }

    #[test]
    fn parses_stable_ids_labels_and_default() {
        let set = ProfileSet::from_json(
            r#"{"default":"b","profiles":[
                {"id":"a","label":"Alpha","url":"postgresql://u:p@h/db"},
                {"id":"b","label":"Beta (MySQL)","url":"mysql://u:p@h/db"}]}"#,
        )
        .unwrap();
        assert_eq!(set.profiles.len(), 2);
        assert_eq!(set.profiles[set.default].id, "b");
        assert_eq!(set.find("a").unwrap().label, "Alpha");
        assert!(set.find("zzz").is_none());
    }

    #[test]
    fn single_url_becomes_the_default_profile() {
        let set = ProfileSet::single(SecretUrl::new("postgresql://h/db".into()));
        assert_eq!(set.profiles[0].id, DEFAULT_PROFILE_ID);
    }

    #[test]
    fn rejects_duplicates_bad_ids_labels_unknown_fields_and_defaults() {
        let ok = |id: &str, label: &str| {
            format!(r#"{{"id":"{id}","label":"{label}","url":"postgresql://h/db"}}"#)
        };
        for (text, why) in [
            (
                doc(&format!("{},{}", ok("a", "One"), ok("a", "Two"))),
                "duplicate id",
            ),
            (
                doc(&format!("{},{}", ok("a", "One"), ok("b", "one"))),
                "duplicate label",
            ),
            (doc(&ok("Has Space", "x")), "bad id"),
            (doc(&ok("-lead", "x")), "leading dash"),
            (doc(&ok(&"a".repeat(33), "x")), "long id"),
            (doc(&ok("a", " padded ")), "outer spaces"),
            (doc(&ok("a", "")), "empty label"),
            (doc(&ok("a", &"x".repeat(49))), "long label"),
            (
                doc(r#"{"id":"a","label":"x","url":"h","extra":1}"#),
                "unknown field",
            ),
            (doc(r#"{"id":"a","label":"x","url":""}"#), "empty url"),
            (r#"{"profiles":[]}"#.into(), "no profiles"),
            (
                r#"{"default":"nope","profiles":[{"id":"a","label":"x","url":"u"}]}"#.into(),
                "unknown default",
            ),
            ("not json".into(), "not json"),
        ] {
            assert!(ProfileSet::from_json(&text).is_err(), "{why}");
        }
        let nine: Vec<String> = (0..9)
            .map(|i| ok(&format!("p{i}"), &format!("P{i}")))
            .collect();
        assert!(
            ProfileSet::from_json(&doc(&nine.join(","))).is_err(),
            "more than 8"
        );
        assert!(
            ProfileSet::from_json(&"x".repeat(70_000)).is_err(),
            "oversized file"
        );
        let eight: Vec<String> = (0..8)
            .map(|i| ok(&format!("p{i}"), &format!("P{i}")))
            .collect();
        assert!(ProfileSet::from_json(&doc(&eight.join(","))).is_ok());
    }

    #[test]
    fn errors_and_debug_output_never_contain_urls_or_passwords() {
        let secret = "hunter2-do-not-log";
        let text = format!(
            r#"{{"profiles":[{{"id":"a","label":"A","url":"postgresql://u:{secret}@h/db"}},{{"id":"a","label":"B","url":"postgresql://u:{secret}@h/db"}}]}}"#
        );
        let error = ProfileSet::from_json(&text).unwrap_err();
        assert!(!error.to_string().contains(secret) && !format!("{error:?}").contains(secret));
        let good = text.replace(r#""id":"a","label":"B""#, r#""id":"b","label":"B""#);
        let set = ProfileSet::from_json(&good).unwrap();
        assert!(
            !format!("{set:?}").contains(secret) && !format!("{set:?}").contains("postgresql://")
        );
        let broken =
            format!(r#"{{"profiles":[{{"id":"a","label":"A","url":"{secret}","oops":1}}]}}"#);
        assert!(
            !ProfileSet::from_json(&broken)
                .unwrap_err()
                .to_string()
                .contains(secret)
        );
    }
}
