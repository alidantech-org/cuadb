//! Strict parsing of operator-controlled PostgreSQL connection URLs.
//!
//! SQLx has liberal libpq defaults (including PG* environment and pgpass).
//! CuaDB requires an explicit host, user and database. Passwords are required
//! outside loopback; a local trust-auth fixture may omit one. TLS and URL
//! parameters remain restricted, and connection settings never enter HTML.

use std::{fmt, str::FromStr};

use sqlx::{
    ConnectOptions,
    postgres::{PgConnectOptions, PgSslMode},
};
use url::{Host, Url};

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
            Self::Password => "must supply a password except for local loopback connections",
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
    // Trust authentication is useful for a single-user development machine.
    // Never permit an omitted/empty password for remote targets, even if the
    // operator configured plaintext TLS. Only literal loopback IPs and the
    // exact hostname "localhost" qualify; other DNS names are not trusted.
    let loopback = match url.host() {
        Some(Host::Domain(host)) => {
            // For non-special URL schemes, the url crate can represent a
            // numeric IPv4 host as Domain rather than Host::Ipv4. Classify
            // numeric loopback addresses explicitly instead of assuming that
            // a Domain variant is never an IP literal.
            host.eq_ignore_ascii_case("localhost")
                || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
        },
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    let passwordless = url.password().is_none_or(str::is_empty);
    if passwordless && !loopback {
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

    // Local trust-auth PostgreSQL commonly has no TLS certificate. Remote
    // targets still require verified TLS unless the operator explicitly opts
    // into another supported mode.
    let mut mode = if passwordless { PgSslMode::Disable } else { PgSslMode::VerifyFull };
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

    // SQLx 0.8.x loads libpq environment defaults and .pgpass during parsing.
    // Explicitly set an empty password in the URL before it reaches SQLx to
    // prevent .pgpass fallback for a passwordless loopback connection.
    let mut driver_url = url;
    if passwordless && driver_url.password().is_none() {
        driver_url
            .set_password(Some(""))
            .map_err(|_| UrlError::Malformed)?;
    }
    let mut options =
        PgConnectOptions::from_str(driver_url.as_str()).map_err(|_| UrlError::Malformed)?;
    if passwordless {
        options = options.password("");
    }
    // An inherited PGOPTIONS value could alter startup/session behavior.
    // Reject it; trusted URLs cannot enable arbitrary connection options.
    if options.get_options().is_some() {
        return Err(UrlError::Parameter);
    }
    options = options.ssl_mode(mode).port(driver_url.port().unwrap_or(5432));
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
    fn accepts_passwordless_local_postgres_only() {
        for (case, raw) in [
            ("hostname", "postgresql://postgres@localhost:5432/postgres"),
            ("ipv4_loopback", "postgres://postgres@127.0.0.1:5432/postgres"),
            ("ipv4_loopback_alias", "postgres://postgres@127.0.0.2:5432/postgres"),
            ("ipv6_loopback", "postgres://postgres@[::1]:5432/postgres"),
            ("explicit_empty_password", "postgres://postgres:@localhost:5432/postgres"),
        ] {
            let options = parse(raw)
                .unwrap_or_else(|error| panic!("{case}: {error}"))
                .into_options();
            assert_eq!(options.get_username(), "postgres");
            assert_eq!(options.get_database(), Some("postgres"));
            assert!(matches!(
                options.get_ssl_mode(),
                sqlx::postgres::PgSslMode::Disable
            ));
        }
    }

    #[test]
    fn rejects_passwordless_non_loopback_hosts() {
        for raw in [
            "postgresql://postgres@db:5432/postgres",
            "postgresql://postgres@postgis:5432/postgres?sslmode=disable",
            "postgresql://postgres@192.168.1.2:5432/postgres",
            "postgresql://postgres:@example.com:5432/postgres",
        ] {
            assert_eq!(parse(raw).unwrap_err(), UrlError::Password);
        }
    }

    #[test]
    fn explicit_tls_settings_override_loopback_defaults() {
        let options = parse(
            "postgresql://postgres@localhost:5432/postgres?sslmode=verify-full",
        )
        .unwrap()
        .into_options();
        assert!(matches!(
            options.get_ssl_mode(),
            sqlx::postgres::PgSslMode::VerifyFull
        ));

        let remote = parse("postgresql://viewer:secret@remote-db:5432/appdb")
            .unwrap()
            .into_options();
        assert!(matches!(
            remote.get_ssl_mode(),
            sqlx::postgres::PgSslMode::VerifyFull
        ));
    }

    #[test]
    fn error_debug_cannot_reveal_password() {
        let result = parse("postgresql://admin:secret123@host/db?options=bad");
        let error = result.unwrap_err();
        assert!(!format!("{error:?}").contains("secret123"));
        assert!(!error.to_string().contains("secret123"));
    }
}
