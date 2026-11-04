//! Strict, redacted MySQL connection URL parsing for operator-owned settings.
use std::{fmt, str::FromStr};

use sqlx::mysql::{MySqlConnectOptions, MySqlSslMode};
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
        let message = match self {
            Self::Malformed => "must be a valid MySQL connection URL",
            Self::Scheme => "must begin with mysql:// or mariadb://",
            Self::Host => "must specify one TCP hostname or IP address",
            Self::Username => "must specify a username",
            Self::Password => "must supply a password except for local loopback connections",
            Self::Database => "must specify one nonempty database name",
            Self::Encoding => "contains invalid percent encoding",
            Self::Parameter => "contains an unsupported or duplicate parameter",
            Self::TlsMode => "has an unsupported or insecure ssl-mode",
        };
        write!(f, "Invalid MySQL CUADB_DATABASE_URL: {message}")
    }
}
impl std::error::Error for UrlError {}

pub struct ValidatedMySqlUrl {
    options: MySqlConnectOptions,
}
impl fmt::Debug for ValidatedMySqlUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ValidatedMySqlUrl([REDACTED])")
    }
}
impl ValidatedMySqlUrl {
    pub fn into_options(self) -> MySqlConnectOptions {
        self.options
    }
}

pub fn parse(raw: &str) -> Result<ValidatedMySqlUrl, UrlError> {
    if !raw.starts_with("mysql://") && !raw.starts_with("mariadb://") {
        return Err(UrlError::Scheme);
    }
    if raw.chars().any(char::is_whitespace) || raw.contains('#') {
        return Err(UrlError::Malformed);
    }
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
    let mut url = Url::parse(raw).map_err(|_| UrlError::Malformed)?;
    if url.host_str().is_none_or(str::is_empty)
        || url.cannot_be_a_base()
        || !url.path().starts_with('/')
    {
        return Err(UrlError::Host);
    }
    if url.username().is_empty() {
        return Err(UrlError::Username);
    }
    let loopback = match url.host() {
        Some(Host::Domain(host)) => {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        }
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    let passwordless = url.password().is_none_or(str::is_empty);
    if passwordless && !loopback {
        return Err(UrlError::Password);
    }
    let path = url.path().strip_prefix('/').ok_or(UrlError::Database)?;
    if path.is_empty()
        || path.contains('/')
        || path.contains('\\')
        || path == "."
        || path == ".."
        || path.to_ascii_lowercase().contains("%2f")
        || url.path_segments().is_none()
    {
        return Err(UrlError::Database);
    }

    let mut ssl_mode = if loopback {
        MySqlSslMode::Disabled
    } else {
        MySqlSslMode::VerifyIdentity
    };
    let mut seen_ssl_mode = false;
    for (key, value) in url.query_pairs() {
        if key != "ssl-mode" || seen_ssl_mode {
            return Err(UrlError::Parameter);
        }
        seen_ssl_mode = true;
        ssl_mode = match value.to_ascii_lowercase().as_str() {
            "disabled" if loopback => MySqlSslMode::Disabled,
            "required" => MySqlSslMode::Required,
            "verify_ca" => MySqlSslMode::VerifyCa,
            "verify_identity" => MySqlSslMode::VerifyIdentity,
            _ => return Err(UrlError::TlsMode),
        };
    }
    if !loopback && matches!(ssl_mode, MySqlSslMode::Disabled) {
        return Err(UrlError::TlsMode);
    }
    if passwordless && url.password().is_none() {
        url.set_password(Some(""))
            .map_err(|_| UrlError::Malformed)?;
    }
    url.set_query(None);
    let driver_url = if url.scheme() == "mariadb" {
        url.as_str().replacen("mariadb://", "mysql://", 1)
    } else {
        url.to_string()
    };
    let options = MySqlConnectOptions::from_str(&driver_url)
        .map_err(|_| UrlError::Malformed)?
        .ssl_mode(ssl_mode);
    Ok(ValidatedMySqlUrl { options })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_and_verified_remote_urls() {
        for raw in [
            "mysql://reader@localhost/appdb",
            "mysql://reader:pass@127.0.0.1/appdb",
            "mariadb://reader:pass@[::1]/appdb",
            "mysql://reader:pass@db.example/appdb",
            "mysql://reader:pass@db.example/appdb?ssl-mode=verify_identity",
        ] {
            assert!(parse(raw).is_ok(), "{raw}");
        }
    }

    #[test]
    fn rejects_untrusted_targets_and_unsupported_options() {
        for (raw, expected) in [
            ("postgres://reader:pass@localhost/appdb", UrlError::Scheme),
            ("mysql://reader:pass@/appdb", UrlError::Malformed),
            (
                "mysql://reader:pass@localhost/appdb/other",
                UrlError::Database,
            ),
            (
                "mysql://reader:pass@localhost/appdb%2fother",
                UrlError::Database,
            ),
            ("mysql://reader@db.example/appdb", UrlError::Password),
            ("mysql://reader:pass@localhost/", UrlError::Database),
            (
                "mysql://reader:pass@localhost/appdb?host=elsewhere",
                UrlError::Parameter,
            ),
            (
                "mysql://reader:pass@db.example/appdb?ssl-mode=disabled",
                UrlError::TlsMode,
            ),
            (
                "mysql://reader:pass@localhost/appdb?ssl-mode=disabled&ssl-mode=required",
                UrlError::Parameter,
            ),
            ("mysql://reader:p%XX@localhost/appdb", UrlError::Encoding),
        ] {
            assert_eq!(parse(raw).unwrap_err(), expected, "{raw}");
        }
    }

    #[test]
    fn options_debug_never_contains_credentials() {
        let options = parse("mysql://reader:secret@db.example/appdb").unwrap();
        assert!(!format!("{options:?}").contains("secret"));
    }
}
