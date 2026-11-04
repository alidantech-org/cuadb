//! Typed, redacted startup configuration for CuaDB.
//!
//! Parse configuration before binding an HTTP listener. Network connections and
//! database queries will use the validated limits when their adapters arrive.

use std::{
    collections::HashMap,
    env, fmt,
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_PORT: u64 = 4848;

/// Credential-bearing input: safe to include in a `Debug` configuration dump.
///
/// Only the future database adapter should call `expose`. Never log the result.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretUrl(String);

impl SecretUrl {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretUrl([REDACTED])")
    }
}

/// Validated resource limits reserved for database and HTTP implementation.
///
/// These values are not enforced on the shell's routes yet; future tasks will
/// use them when adding PostgreSQL pools, paging and request timeouts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub connection_timeout: Duration,
    pub query_timeout: Duration,
    pub request_timeout: Duration,
    pub default_page_size: usize,
    pub max_page_size: usize,
    pub max_pool_connections: u32,
    pub max_in_flight_requests: usize,
    pub max_request_body_bytes: usize,
}

/// Typed settings. The optional connection URL is intentionally private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub limits: Limits,
    database_url: Option<SecretUrl>,
}

/// An invalid setting with a useful, secret-free error message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    name: &'static str,
    requirement: &'static str,
}

impl ConfigError {
    const fn invalid(name: &'static str, requirement: &'static str) -> Self {
        Self { name, requirement }
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Invalid {}: {}", self.name, self.requirement)
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Read known environment variables. Invalid Unicode never enters errors.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::parse_with(|name| match env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(env::VarError::NotUnicode(_)) => {
                Err(ConfigError::invalid(name, "must be valid UTF-8"))
            }
        })
    }

    /// Parse settings without mutating global environment variables.
    ///
    /// This pure entry point makes malformed configuration easy to test.
    pub fn from_values(values: &[(&str, &str)]) -> Result<Self, ConfigError> {
        let values: HashMap<&str, &str> = values.iter().copied().collect();
        Self::parse_with(|name| Ok(values.get(name).map(|value| (*value).to_owned())))
    }

    /// Retain the shell's previous convenience constructor.
    pub fn from_port(port: &str) -> Result<Self, ConfigError> {
        Self::from_values(&[("CUADB_PORT", port)])
    }

    pub fn database_url(&self) -> Option<&SecretUrl> {
        self.database_url.as_ref()
    }

    fn parse_with(
        mut read: impl FnMut(&'static str) -> Result<Option<String>, ConfigError>,
    ) -> Result<Self, ConfigError> {
        let host = match read("CUADB_HOST")? {
            Some(value) => value.parse().map_err(|_| {
                ConfigError::invalid("CUADB_HOST", "must be an IPv4 or IPv6 address")
            })?,
            None => DEFAULT_HOST,
        };
        let port = bounded_number(
            &mut read,
            "CUADB_PORT",
            DEFAULT_PORT,
            1,
            65_535,
            "must be a whole number between 1 and 65535",
        )? as u16;

        let connection_timeout = Duration::from_millis(bounded_number(
            &mut read,
            "CUADB_CONNECTION_TIMEOUT_MS",
            5_000,
            100,
            60_000,
            "must be a whole number between 100 and 60000 milliseconds",
        )?);
        let query_timeout = Duration::from_millis(bounded_number(
            &mut read,
            "CUADB_QUERY_TIMEOUT_MS",
            15_000,
            100,
            120_000,
            "must be a whole number between 100 and 120000 milliseconds",
        )?);
        let request_timeout = Duration::from_millis(bounded_number(
            &mut read,
            "CUADB_REQUEST_TIMEOUT_MS",
            30_000,
            500,
            180_000,
            "must be a whole number between 500 and 180000 milliseconds",
        )?);

        let max_page_size = bounded_number(
            &mut read,
            "CUADB_MAX_PAGE_SIZE",
            100,
            1,
            500,
            "must be a whole number between 1 and 500",
        )? as usize;
        let default_page_size = bounded_number(
            &mut read,
            "CUADB_DEFAULT_PAGE_SIZE",
            25_u64.min(max_page_size as u64),
            1,
            500,
            "must be a whole number between 1 and 500",
        )? as usize;
        if default_page_size > max_page_size {
            return Err(ConfigError::invalid(
                "CUADB_DEFAULT_PAGE_SIZE",
                "must not exceed CUADB_MAX_PAGE_SIZE",
            ));
        }
        let max_pool_connections = bounded_number(
            &mut read,
            "CUADB_MAX_POOL_CONNECTIONS",
            4,
            1,
            32,
            "must be a whole number between 1 and 32",
        )? as u32;

        // URL scheme/host/TLS validation belongs to the PostgreSQL URL task.
        // Here we only reject empty or clearly excessive credential input.
        let max_in_flight_requests = bounded_number(
            &mut read,
            "CUADB_MAX_IN_FLIGHT_REQUESTS",
            64,
            1,
            512,
            "must be a whole number between 1 and 512",
        )? as usize;
        let max_request_body_bytes = bounded_number(
            &mut read,
            "CUADB_MAX_REQUEST_BODY_BYTES",
            65_536,
            1_024,
            1_048_576,
            "must be a whole number between 1024 and 1048576",
        )? as usize;

        let database_url = match read("CUADB_DATABASE_URL")? {
            Some(value) if value.is_empty() || value.len() > 8192 || value.trim() != value => {
                return Err(ConfigError::invalid(
                    "CUADB_DATABASE_URL",
                    "must be nonempty, at most 8192 bytes and have no outer whitespace",
                ));
            }
            Some(value) => Some(SecretUrl(value)),
            None => None,
        };

        Ok(Self {
            host,
            port,
            limits: Limits {
                connection_timeout,
                query_timeout,
                request_timeout,
                default_page_size,
                max_page_size,
                max_pool_connections,
                max_in_flight_requests,
                max_request_body_bytes,
            },
            database_url,
        })
    }
}

/// The error includes the key and requirement, but never the supplied value.
fn bounded_number(
    read: &mut impl FnMut(&'static str) -> Result<Option<String>, ConfigError>,
    name: &'static str,
    default: u64,
    min: u64,
    max: u64,
    requirement: &'static str,
) -> Result<u64, ConfigError> {
    let number = match read(name)? {
        Some(raw) => raw
            .parse::<u64>()
            .map_err(|_| ConfigError::invalid(name, requirement))?,
        None => default,
    };
    if number < min || number > max {
        return Err(ConfigError::invalid(name, requirement));
    }
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::{net::IpAddr, time::Duration};

    #[test]
    fn shell_defaults_require_no_database() {
        let config = Config::from_values(&[]).unwrap();
        assert_eq!(config.host, IpAddr::from([127, 0, 0, 1]));
        assert_eq!(config.port, 4848);
        assert!(config.database_url().is_none());
        assert_eq!(config.limits.connection_timeout, Duration::from_secs(5));
        assert_eq!(config.limits.query_timeout, Duration::from_secs(15));
        assert_eq!(config.limits.request_timeout, Duration::from_secs(30));
        assert_eq!(config.limits.default_page_size, 25);
        assert_eq!(config.limits.max_page_size, 100);
        assert_eq!(config.limits.max_pool_connections, 4);
        assert_eq!(config.limits.max_in_flight_requests, 64);
        assert_eq!(config.limits.max_request_body_bytes, 65_536);
    }

    #[test]
    fn supports_custom_ipv6_host_and_bounded_limits() {
        let config = Config::from_values(&[
            ("CUADB_HOST", "::1"),
            ("CUADB_PORT", "8100"),
            ("CUADB_CONNECTION_TIMEOUT_MS", "2000"),
            ("CUADB_QUERY_TIMEOUT_MS", "3000"),
            ("CUADB_REQUEST_TIMEOUT_MS", "4500"),
            ("CUADB_MAX_PAGE_SIZE", "20"),
            ("CUADB_MAX_POOL_CONNECTIONS", "2"),
        ])
        .unwrap();

        assert_eq!(config.host, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(config.port, 8100);
        assert_eq!(config.limits.connection_timeout, Duration::from_secs(2));
        assert_eq!(config.limits.query_timeout, Duration::from_secs(3));
        assert_eq!(config.limits.request_timeout, Duration::from_millis(4500));
        assert_eq!(config.limits.default_page_size, 20);
        assert_eq!(config.limits.max_page_size, 20);
        assert_eq!(config.limits.max_pool_connections, 2);
    }

    #[test]
    fn rejects_bad_host_port_timeouts_and_limits() {
        for (key, value) in [
            ("CUADB_HOST", "localhost"),
            ("CUADB_HOST", "not-an-ip"),
            ("CUADB_PORT", "0"),
            ("CUADB_PORT", "65536"),
            ("CUADB_PORT", "-1"),
            ("CUADB_PORT", "nonsense"),
            ("CUADB_CONNECTION_TIMEOUT_MS", "0"),
            ("CUADB_QUERY_TIMEOUT_MS", "120001"),
            ("CUADB_REQUEST_TIMEOUT_MS", "499"),
            ("CUADB_MAX_PAGE_SIZE", "501"),
            ("CUADB_DEFAULT_PAGE_SIZE", "0"),
            ("CUADB_MAX_POOL_CONNECTIONS", "33"),
            ("CUADB_MAX_IN_FLIGHT_REQUESTS", "0"),
            ("CUADB_MAX_REQUEST_BODY_BYTES", "999999999"),
        ] {
            let error = Config::from_values(&[(key, value)]).unwrap_err();
            assert_eq!(error.name(), key);
        }

        let error = Config::from_values(&[
            ("CUADB_MAX_PAGE_SIZE", "20"),
            ("CUADB_DEFAULT_PAGE_SIZE", "100"),
        ])
        .unwrap_err();
        assert_eq!(error.name(), "CUADB_DEFAULT_PAGE_SIZE");
    }

    #[test]
    fn redacts_database_credentials_from_config_debug_output() {
        let secret = "do-not-log-this-password";
        let url = format!("postgresql://viewer:{secret}@postgis:5432/app");
        let config = Config::from_values(&[("CUADB_DATABASE_URL", &url)]).unwrap();
        assert_eq!(config.database_url().unwrap().expose(), url);

        let debug = format!("{config:?}");
        assert!(!debug.contains(secret));
        assert!(!debug.contains("postgresql://"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn errors_cannot_contain_supplied_secret_values() {
        let secret = "never-echo-this-secret";
        let url = format!("postgresql://viewer:{secret}@postgis:5432/app");
        let error = Config::from_values(&[("CUADB_PORT", &url)]).unwrap_err();
        assert_eq!(error.name(), "CUADB_PORT");
        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));

        let too_long = "secret".repeat(1500);
        let error = Config::from_values(&[("CUADB_DATABASE_URL", &too_long)]).unwrap_err();
        assert_eq!(error.name(), "CUADB_DATABASE_URL");
        assert!(!error.to_string().contains("secret"));
        assert!(Config::from_values(&[("CUADB_DATABASE_URL", "")]).is_err());
    }

    #[test]
    fn previous_port_constructor_remains_supported() {
        assert_eq!(Config::from_port("8080").unwrap().port, 8080);
        assert!(Config::from_port("65536").is_err());
        assert!(Config::from_port("not-a-port").is_err());
    }
}
