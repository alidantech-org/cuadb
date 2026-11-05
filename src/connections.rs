//! Building and reading the URLs of connections saved from the Administration screen.
//!
//! The browser sends separate fields (host, user, password...). They are assembled here
//! into one URL that only the encrypted store and the engine's own validator ever see.
//! Messages are short, static and user-correctable; they never echo a value.

use url::Url;

use crate::profiles::valid_id;

pub const MAX_LABEL_CHARS: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    PostgreSql,
    Sqlite,
}

impl Engine {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "postgresql" => Some(Self::PostgreSql),
            "sqlite" => Some(Self::Sqlite),
            _ => None,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::PostgreSql => "postgresql",
            Self::Sqlite => "sqlite",
        }
    }
}

/// What the form carries. `password` is `None` when left blank on an edit.
#[derive(Debug, Default, Clone)]
pub struct Fields {
    pub engine: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: Option<String>,
    pub database: String,
    pub tls: String,
    pub file: String,
}

/// Lowercase slug usable as a profile id (`Production DB` becomes `production-db`).
pub fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    trimmed
        .chars()
        .take(32)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

pub fn valid_label(label: &str) -> bool {
    let trimmed = label.trim();
    !trimmed.is_empty()
        && trimmed == label
        && label.chars().count() <= MAX_LABEL_CHARS
        && !label.chars().any(char::is_control)
}

pub fn check_id(id: &str) -> Result<(), &'static str> {
    if valid_id(id) {
        Ok(())
    } else {
        Err("The name must contain letters or digits so it can become an id")
    }
}

fn host_ok(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':' | b'[' | b']')
        })
}

/// Assemble the engine URL. `sqlite_root` is only used for SQLite files.
pub fn build_url(fields: &Fields, sqlite_root: &std::path::Path) -> Result<String, &'static str> {
    let mut fields = fields.clone();
    if fields.engine == "postgresql" && !fields.file.is_empty() {
        if fields.host.is_empty() && fields.user.is_empty() && fields.database.is_empty() {
            let parsed = validate_pasted_url(&fields.file)?;
            fields.host = parsed.host;
            fields.port = parsed.port;
            fields.user = parsed.user;
            fields.password = parsed.password;
            fields.database = parsed.database;
            fields.tls = parsed.tls;
        } else {
            return Err("Choose either a connection URL or enter credentials below");
        }
    }
    match Engine::parse(&fields.engine).ok_or("Choose PostgreSQL or SQLite")? {
        Engine::Sqlite => {
            let name = fields.file.trim();
            if name.is_empty() || name.len() > 255 || name.contains(['\\', '\0', '?', '#', '%']) {
                return Err("Enter the database file name");
            }
            if name.starts_with('/') || name.split('/').any(|part| part == ".." || part.is_empty())
            {
                return Err("The file must be inside the SQLite folder");
            }
            let root =
                std::path::absolute(sqlite_root).unwrap_or_else(|_| sqlite_root.to_path_buf());
            let full = root.join(name);
            Ok(format!(
                "sqlite:///{}",
                full.display().to_string().trim_start_matches('/')
            ))
        }
        Engine::PostgreSql => {
            let host = fields.host.trim();
            if !host_ok(host) {
                return Err("Enter a valid host name or address");
            }
            let port: u16 = if fields.port.trim().is_empty() {
                5432
            } else {
                fields
                    .port
                    .trim()
                    .parse()
                    .ok()
                    .filter(|p| *p > 0)
                    .ok_or("The port must be a number from 1 to 65535")?
            };
            let user = fields.user.trim();
            if user.is_empty() || user.chars().any(char::is_control) {
                return Err("Enter the user name");
            }
            let database = fields.database.trim();
            if database.is_empty()
                || database.len() > 63
                || database.chars().any(char::is_control)
                || database.contains('/')
            {
                return Err("Enter the database name");
            }
            let loopback = host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback());
            let tls = match fields.tls.as_str() {
                "" | "auto" if loopback => "disable",
                "" | "auto" | "verify-full" => "verify-full",
                "require" => "require",
                "disable" => "disable",
                _ => return Err("Choose a TLS mode from the list"),
            };
            let mut url = Url::parse("postgresql://placeholder/").map_err(|_| "Invalid address")?;
            url.set_host(Some(host))
                .map_err(|_| "Enter a valid host name or address")?;
            url.set_port(Some(port)).map_err(|()| "Invalid port")?;
            url.set_username(user).map_err(|()| "Invalid user name")?;
            if let Some(password) = fields.password.as_deref().filter(|p| !p.is_empty()) {
                if password.len() > 256 || password.chars().any(char::is_control) {
                    return Err("The password contains characters that are not allowed");
                }
                url.set_password(Some(password))
                    .map_err(|()| "Invalid password")?;
            }
            url.set_path(database);
            url.query_pairs_mut().append_pair("sslmode", tls);
            Ok(url.into())
        }
    }
}

fn validate_pasted_url(raw: &str) -> Result<Fields, &'static str> {
    if !raw.starts_with("postgres://") && !raw.starts_with("postgresql://") {
        return Err("Paste a PostgreSQL URL beginning with postgres:// or postgresql://");
    }
    let parsed = Url::parse(raw).map_err(|_| "Enter a valid PostgreSQL URL")?;
    if parsed.host_str().is_none_or(str::is_empty) || parsed.username().is_empty() {
        return Err("The URL must include a host and user name");
    }
    let database = parsed.path().trim_start_matches('/');
    if database.is_empty() || database.contains('/') {
        return Err("The URL must include one database name");
    }
    let tls = parsed
        .query_pairs()
        .find(|(key, _)| key == "sslmode")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "verify-full".into());
    if !matches!(
        tls.as_str(),
        "disable" | "require" | "verify-ca" | "verify-full"
    ) {
        return Err("Choose a supported TLS mode in the PostgreSQL URL");
    }
    Ok(Fields {
        engine: "postgresql".into(),
        host: parsed.host_str().unwrap_or_default().into(),
        port: parsed.port().unwrap_or(5432).to_string(),
        user: parsed.username().into(),
        password: parsed.password().map(str::to_string),
        database: database.into(),
        tls,
        ..Fields::default()
    })
}

/// The non-secret parts of a stored URL, to prefill the edit form. The password is
/// deliberately absent.
pub fn read_fields(url: &str, sqlite_root: &std::path::Path) -> Fields {
    if let Some(rest) = url.strip_prefix("sqlite:///") {
        let path = std::path::Path::new("/").join(rest);
        let name = path
            .strip_prefix(sqlite_root)
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        return Fields {
            engine: "sqlite".into(),
            file: name,
            ..Fields::default()
        };
    }
    let Ok(parsed) = Url::parse(url) else {
        return Fields::default();
    };
    Fields {
        engine: "postgresql".into(),
        host: parsed.host_str().unwrap_or_default().to_string(),
        port: parsed.port().map(|p| p.to_string()).unwrap_or_default(),
        user: parsed.username().to_string(),
        password: None,
        database: parsed.path().trim_start_matches('/').to_string(),
        tls: parsed
            .query_pairs()
            .find(|(k, _)| k == "sslmode")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default(),
        file: String::new(),
    }
}

/// The saved password of a stored URL (used when an edit leaves the field blank).
pub fn saved_password(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|u| u.password().map(percent_decode))
}

fn percent_decode(text: &str) -> String {
    url::form_urlencoded::parse(format!("v={}", text.replace('+', "%2B")).as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pg(host: &str, user: &str, password: &str) -> Fields {
        Fields {
            engine: "postgresql".into(),
            host: host.into(),
            port: "5433".into(),
            user: user.into(),
            password: Some(password.into()),
            database: "appdb".into(),
            tls: "require".into(),
            file: String::new(),
        }
    }

    #[test]
    fn postgres_urls_encode_special_characters_and_round_trip() {
        let root = std::path::Path::new("/data");
        let url = build_url(&pg("db.internal", "app@x", "p@ss:w/rd#1+"), root).unwrap();
        assert!(url.starts_with("postgresql://app%40x:"));
        assert!(url.ends_with("/appdb?sslmode=require"));
        assert_eq!(saved_password(&url).as_deref(), Some("p@ss:w/rd#1+"));
        let fields = read_fields(&url, root);
        assert_eq!(
            (
                fields.host.as_str(),
                fields.port.as_str(),
                fields.database.as_str()
            ),
            ("db.internal", "5433", "appdb")
        );
        assert!(
            fields.password.is_none(),
            "the password is never read back for display"
        );
    }

    #[test]
    fn rejects_bad_hosts_ports_and_tls_without_echoing_them() {
        let root = std::path::Path::new("/data");
        for (mutate, expect) in [
            (
                Fields {
                    host: "bad host!".into(),
                    ..pg("x", "u", "p")
                },
                "host",
            ),
            (
                Fields {
                    port: "70000".into(),
                    ..pg("x", "u", "p")
                },
                "port",
            ),
            (
                Fields {
                    tls: "prefer".into(),
                    ..pg("x", "u", "p")
                },
                "TLS",
            ),
            (
                Fields {
                    database: "a/b".into(),
                    ..pg("x", "u", "p")
                },
                "database",
            ),
        ] {
            let message = build_url(&mutate, root).unwrap_err();
            assert!(message.contains(expect), "{message}");
            assert!(!message.contains("bad host") && !message.contains("70000"));
        }
    }

    #[test]
    fn sqlite_files_must_stay_inside_the_data_folder() {
        let root = std::path::Path::new("/data");
        let ok = Fields {
            engine: "sqlite".into(),
            file: "shop/app.db".into(),
            ..Fields::default()
        };
        assert_eq!(build_url(&ok, root).unwrap(), "sqlite:///data/shop/app.db");
        assert_eq!(
            read_fields("sqlite:///data/shop/app.db", root).file,
            "shop/app.db"
        );
        for bad in ["../x.db", "/etc/passwd", "a//b.db", "a?mode=rwc", ""] {
            let f = Fields {
                engine: "sqlite".into(),
                file: bad.into(),
                ..Fields::default()
            };
            assert!(build_url(&f, root).is_err(), "{bad}");
        }
    }

    #[test]
    fn slugs_become_valid_ids() {
        assert_eq!(slug("Production DB (EU)"), "production-db-eu");
        assert!(check_id(&slug("Prod")).is_ok());
        assert!(check_id(&slug("!!!")).is_err());
    }
}
