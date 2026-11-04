//! Environment-based configuration for the self-contained HTTP server.

/// The app deliberately starts without database credentials during scaffolding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    pub host: std::net::IpAddr,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let port = std::env::var("CUADB_PORT").unwrap_or_else(|_| "4848".to_owned());
        let host = std::env::var("CUADB_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());

        let mut config = Self::from_port(&port)?;
        config.host = host.parse()?;
        Ok(config)
    }

    /// Pure parser, independently testable without modifying process environment.
    pub fn from_port(value: &str) -> Result<Self, std::num::ParseIntError> {
        Ok(Self {
            port: value.parse()?,
            host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn parses_custom_port() {
        assert_eq!(Config::from_port("8080").unwrap().port, 8080);
    }

    #[test]
    fn rejects_out_of_range_port() {
        assert!(Config::from_port("65536").is_err());
    }

    #[test]
    fn rejects_non_numeric_port() {
        assert!(Config::from_port("not-a-port").is_err());
    }
}
