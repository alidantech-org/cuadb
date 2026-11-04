//! Strict parsing of operator-controlled PostgreSQL connection URLs.
//!
//! SQLx has liberal libpq defaults (including PG* environment and pgpass).
//! CuaDB deliberately requires an explicit network host, user, password and
//! database, and allows only a small TLS query-option set.

use std::{fmt, str::FromStr};

use sqlx::{
    ConnectOptions,
    postgres::{PgConnectOptions, PgSslMode},
};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlError {
    Malformed,
    Scheme,
    Host,
    Username,
    Password,
    Database,
    Encoding,
    Parameter,
    TlsMode,
}

impl fmt::Display for UrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let requirement = match self {
            Self::Malformed => "must be a valid PostgreSQL URL",
            Self::Scheme => "must begin with postgres:// or postgresql://",
            Self::Host => "must specify one TCP hostname or IP address",
            Self::Username => "must specify a username",
            Self::Password => "must specify a password in the operator configuration",
            Self::Database => "must specify one nonempty database name",
            Self::Encoding => "contains an invalid percent-encoded character",
            Self::Parameter => "contains an unsupported or duplicate connection parameter",
            Self::TlsMode => "has an unsupported sslmode value",
        };
        write!(f, "Invalid CUADB_DATABASE_URL: {requirement}")
    }
}
impl std::error::Error for UrlError {}

pub struct ValidatedPgUrl {
    options: PgConnectOptions,
}

impl fmt::Debug for ValidatedPgUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ValidatedPgUrl([REDACTED])")
    }
}

impl ValidatedPgUrl {
    /// Driver settings, intentionally kept away from web and logger modules.
    pub fn into_options(self) -> PgConnectOptions {
        self.options
    }
}

/// Validate the full URL before asking SQLx to parse it. No error echoes input.
pub fn parse(raw: &str) -> Result<ValidatedPgUrl, UrlError> {
    if !raw.starts_with("postgres://") && !raw.starts_with("postgresql://") {
        return Err(UrlError::Scheme);
    }
    if raw.chars().any(char::is_whitespace) || raw.contains('#') {
        return Err(UrlError::Malformed);
    }

    // url::Url may normalize malformed '%' syntax; reject it ourselves.
    let bytes = raw.as_bytes();
    for (offset, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && (offset + 2 >= bytes.len()
                || !bytes[offset + 1].is_ascii_hexdigit()
                || !bytes[offset + 2].is_ascii_hexdigit())
        {
            return Err(UrlError::Encoding);
        }
    }

    let url = Url::parse(raw).map_err(|_| UrlError::Malformed)?;
    if url.host_str().is_none_or(str::is_empty) || url.cannot_be_a_base() {
        return Err(UrlError::Host);
    }
    if url.username().is_empty() {
        return Err(UrlError::Username);
    }
    if url.password().is_none_or(str::is_empty) {
        return Err(UrlError::Password);
    }

    let segments: Vec<_> = url.path_segments().ok_or(UrlError::Database)?.collect();
    if segments.len() != 1 || segments[0].is_empty() || segments[0] == "." || segments[0] == ".." {
        return Err(UrlError::Database);
    }
    // PostgreSQL identifiers may be encoded, but NUL and slash do not make
    // useful database names in this app's explicit single-database model.
    let lower = segments[0].to_ascii_lowercase();
    if lower.contains("%00") || lower.contains("%2f") || lower.contains("%5c") {
        return Err(UrlError::Database);
    }

    let mut mode = PgSslMode::VerifyFull;
    let mut seen_mode = false;
    let mut seen_cert = false;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "sslmode" if !seen_mode => {
                seen_mode = true;
                mode = match value.as_ref() {
                    "disable" => PgSslMode::Disable,
                    "require" => PgSslMode::Require,
                    "verify-ca" => PgSslMode::VerifyCa,
                    "verify-full" => PgSslMode::VerifyFull,
                    _ => return Err(UrlError::TlsMode),
                };
            }
            "sslrootcert" if !seen_cert => {
                seen_cert = true;
                if value.is_empty() || value.len() > 4096 {
                    return Err(UrlError::Parameter);
                }
            }
            _ => return Err(UrlError::Parameter),
        }
    }
    if seen_cert && matches!(mode, PgSslMode::Disable) {
        return Err(UrlError::TlsMode);
    }

    let mut options = PgConnectOptions::from_str(raw).map_err(|_| UrlError::Malformed)?;
    if !seen_mode {
        options = options.ssl_mode(mode);
    }
    // Credentials and SQL are never passed to SQLx's query logger.
    options = options
        .application_name("cuadb")
        .disable_statement_logging();
    Ok(ValidatedPgUrl { options })
}

#[cfg(test)]
mod tests {
    use super::{UrlError, parse};

    #[test]
    fn accepts_strict_explicit_urls_and_encoded_passwords() {
        for value in [
            "postgresql://viewer:p%40ss@postgis:5432/appdb?sslmode=disable",
            "postgres://viewer:p%25ss@localhost:5432/appdb?sslmode=verify-full",
            "postgresql://viewer:secret@[::1]:5432/mydb?sslmode=require",
        ] {
            assert!(parse(value).is_ok(), "valid URL should be accepted");
        }
    }

    #[test]
    fn rejects_unsafe_or_ambiguous_connection_options() {
        for (url, expected) in [
            ("mysql://viewer:pass@db/test", UrlError::Scheme),
            ("postgres://viewer:pass@/db", UrlError::Malformed),
            ("postgres://:pass@db/test", UrlError::Username),
            ("postgres://viewer@db/test", UrlError::Password),
            ("postgres://viewer:pass@db/", UrlError::Database),
            ("postgres://viewer:pass@db/db/other", UrlError::Database),
            (
                "postgres://viewer:pass@db/db?options=-c%20default_transaction_read_only%3Doff",
                UrlError::Parameter,
            ),
            (
                "postgres://viewer:pass@db/db?sslmode=prefer",
                UrlError::TlsMode,
            ),
            (
                "postgres://viewer:pass@db/db?sslmode=disable&sslrootcert=ca.crt",
                UrlError::TlsMode,
            ),
            ("postgres://viewer:p%XX@db/db", UrlError::Encoding),
            (
                "postgres://viewer:pass@db/db?sslmode=require&sslmode=disable",
                UrlError::Parameter,
            ),
            ("postgres://viewer:pass@db/db#fragment", UrlError::Malformed),
        ] {
            assert_eq!(parse(url).unwrap_err(), expected);
        }
    }

    #[test]
    fn error_debug_cannot_reveal_password() {
        let result = parse("postgresql://admin:secret123@host/db?options=bad");
        let error = result.unwrap_err();
        assert!(!format!("{error:?}").contains("secret123"));
        assert!(!error.to_string().contains("secret123"));
    }
}
