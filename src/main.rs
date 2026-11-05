//! The `cuadb` command: start the server (with a small CLI), run admin commands, or
//! check health for containers. Shutdown is bounded.

use cuadb::{app, config::Config, state::AppState};
use std::{error::Error, time::Duration};

const HELP: &str = "\
CuaDB - a local-first database explorer and SQL workspace (PostgreSQL and SQLite).

USAGE:
    cuadb [OPTIONS]
    cuadb admin <bootstrap|reset-password|list-users> [--login NAME] [--password-stdin]
    cuadb healthcheck

OPTIONS:
    -p, --port <PORT>            Port to listen on (default 4848)        [CUADB_PORT]
        --host <IP>              Address to bind (default 127.0.0.1)     [CUADB_HOST]
        --data-dir <DIR>         Where CuaDB keeps its own state         [CUADB_DATA_DIR]
        --sqlite-root <DIR>      Folder SQLite files are opened from     [CUADB_SQLITE_ROOT]
        --database-url <URL>     Add one connection at startup           [CUADB_DATABASE_URL]
        --profiles-file <FILE>   JSON file of several connections        [CUADB_PROFILES_FILE]
        --local                  Trusted local mode: full access, no sign-in
        --no-local               Permissions mode: sign-in and grants
        --open                   Open the browser when the server is ready
    -V, --version                Print the version
    -h, --help                   Print this help

By default the server listens on 127.0.0.1 only, which is treated as trusted local mode.
Every setting is also available as an environment variable (see docs/configuration.md).
";

struct Cli {
    open: bool,
    healthcheck: bool,
    admin: bool,
}

fn set_env(key: &str, value: &str) {
    // SAFETY: called while parsing arguments, before any thread is started.
    unsafe { std::env::set_var(key, value) };
}

/// Where an installed `cuadb` keeps its state when no data directory is given.
/// `cargo run` (development) keeps using `./data`; containers set the variables themselves.
fn default_data_dir() -> Option<std::path::PathBuf> {
    if std::env::var_os("CARGO_MANIFEST_DIR").is_some() {
        return None;
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join("cuadb"));
    }
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/cuadb"));
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")))
        .map(|d| d.join("cuadb"))
}

fn parse_cli() -> Result<Cli, Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cli = Cli {
        open: false,
        healthcheck: false,
        admin: false,
    };
    match args.first().map(String::as_str) {
        Some("admin") => {
            cli.admin = true;
            return Ok(cli);
        }
        Some("healthcheck") => {
            cli.healthcheck = true;
            return Ok(cli);
        }
        _ => {}
    }
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let mut value = |name: &str| -> Result<String, Box<dyn Error>> {
            inline
                .clone()
                .or_else(|| iter.next())
                .ok_or_else(|| format!("{name} needs a value").into())
        };
        match flag.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("cuadb {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-p" | "--port" => set_env("CUADB_PORT", &value("--port")?),
            "--host" => set_env("CUADB_HOST", &value("--host")?),
            "--data-dir" => set_env("CUADB_DATA_DIR", &value("--data-dir")?),
            "--sqlite-root" => set_env("CUADB_SQLITE_ROOT", &value("--sqlite-root")?),
            "--database-url" => set_env("CUADB_DATABASE_URL", &value("--database-url")?),
            "--profiles-file" => set_env("CUADB_PROFILES_FILE", &value("--profiles-file")?),
            "--local" => set_env("CUADB_LOCAL_MODE", "true"),
            "--no-local" => set_env("CUADB_LOCAL_MODE", "false"),
            "--open" => cli.open = true,
            other => {
                return Err(format!("unknown option `{other}`. Try `cuadb --help`.").into());
            }
        }
    }
    if std::env::var_os("CUADB_DATA_DIR").is_none()
        && let Some(dir) = default_data_dir()
    {
        set_env("CUADB_DATA_DIR", &dir.to_string_lossy());
        if std::env::var_os("CUADB_SQLITE_ROOT").is_none() {
            set_env("CUADB_SQLITE_ROOT", &dir.join("sqlite").to_string_lossy());
        }
    }
    Ok(cli)
}

/// Exit status for `cuadb healthcheck` (used by the container HEALTHCHECK).
fn healthcheck() -> i32 {
    use std::{
        io::{Read, Write},
        net::{SocketAddr, TcpStream},
    };
    let port: u16 = std::env::var("CUADB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4848);
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_secs(2)) else {
        return 1;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return 1;
    }
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    i32::from(!reply.starts_with("HTTP/1.0 200") && !reply.starts_with("HTTP/1.1 200"))
}

fn open_browser(url: &str) {
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = parse_cli().inspect_err(|error| eprintln!("cuadb: {error}"))?;
    if cli.healthcheck {
        std::process::exit(healthcheck());
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(cli))
}

async fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    if cli.admin {
        return admin_command(&config).await;
    }
    let state = AppState::from_config(&config)
        .await?
        .with_auth_from_config(&config)
        .await?;
    if state.auth().is_none() {
        eprintln!(
            "level=warn event=auth_not_configured message=\"no administrator exists; this CuaDB instance is in shared public mode and anyone who can reach this port can manage saved connections; restrict network access before exposing it\""
        );
    }
    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    let router = app::router_with_state(&config.limits, state.clone());
    state.start_probe();
    // Roll back expired or orphaned write transactions (their connections are released).
    {
        let sweeper = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                let _ = sweeper.sweep_transactions().await;
            }
        });
    }

    println!(
        "CuaDB shell ready at http://{}:{}",
        config.host, config.port
    );
    if cli.open {
        let shown = if config.host.is_unspecified() {
            "127.0.0.1".to_string()
        } else {
            config.host.to_string()
        };
        open_browser(&format!("http://{shown}:{}", config.port));
    }

    // Notify Axum to stop accepting new connections, then let in-flight
    // handlers complete. The outer deadline prevents a hung shutdown.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
    });

    tokio::select! {
        result = &mut server => {
            result??;
        }
        _ = termination_signal() => {
            eprintln!("level=info event=server_shutdown phase=drain");
            let _ = shutdown_tx.send(());
            // A second Ctrl+C / SIGTERM during the drain exits immediately.
            tokio::spawn(async {
                termination_signal().await;
                eprintln!("level=warn event=server_shutdown phase=forced");
                std::process::exit(130);
            });
            match tokio::time::timeout(shutdown_grace(), &mut server).await {
                Ok(result) => { result??; }
                Err(_) => {
                    server.abort();
                    let _ = server.await;
                    eprintln!("level=warn event=server_shutdown phase=deadline_exceeded");
                }
            }
        }
    }
    // Close checked-out connection resources after the listener has drained.
    if tokio::time::timeout(Duration::from_secs(2), state.close())
        .await
        .is_err()
    {
        eprintln!("level=warn event=database_pool_close code=deadline_exceeded");
    }
    Ok(())
}

/// How long in-flight and keep-alive connections may delay shutdown. Defaults to
/// 8 s (Docker's 10 s stop budget); `CUADB_SHUTDOWN_GRACE_MS` (100-30000) lowers it,
/// which the dev script uses so a restart never leaves the old process holding the port.
fn shutdown_grace() -> Duration {
    let millis = std::env::var("CUADB_SHUTDOWN_GRACE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map_or(8_000, |value| value.clamp(100, 30_000));
    Duration::from_millis(millis)
}

/// SIGINT covers Ctrl+C. SIGTERM is used by Docker on Unix platforms.
async fn termination_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
        } else {
            eprintln!("level=warn event=signal_registration_failed kind=sigterm");
            let _ = tokio::signal::ctrl_c().await;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// `cuadb admin bootstrap|reset-password --login <name> --password-stdin` and
/// `cuadb admin list-users`. Local operator commands; secrets are read from stdin
/// only and are never printed or logged.
async fn admin_command(config: &Config) -> Result<(), Box<dyn Error>> {
    use cuadb::auth::{AuthStore, bootstrap};
    use std::io::Read;

    let args: Vec<String> = std::env::args().skip(2).collect();
    let action = args.first().map(String::as_str).unwrap_or_default();
    let login = args
        .iter()
        .position(|a| a == "--login")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let from_stdin = args.iter().any(|a| a == "--password-stdin");
    let read_password = || -> Result<String, Box<dyn Error>> {
        if !from_stdin {
            return Err("pass the password on standard input with --password-stdin".into());
        }
        let mut raw = String::new();
        std::io::stdin().read_to_string(&mut raw)?;
        Ok(raw
            .strip_suffix('\n')
            .unwrap_or(&raw)
            .trim_end_matches('\r')
            .to_string())
    };
    match action {
        "bootstrap" => {
            let login = login.ok_or("--login <name> is required")?;
            let password = read_password()?;
            let store = AuthStore::open(&config.data_dir, true).await?;
            bootstrap::bootstrap_admin(&store, &login, &password).await?;
            store.close().await;
            println!(
                "Administrator created. Authentication is now enforced for this data directory."
            );
        }
        "reset-password" => {
            let login = login.ok_or("--login <name> is required")?;
            let password = read_password()?;
            let store = AuthStore::open(&config.data_dir, false).await?;
            bootstrap::reset_password(&store, &login, &password).await?;
            store.close().await;
            println!("Password reset, account enabled and sessions revoked.");
        }
        "list-users" => {
            let store = AuthStore::open(&config.data_dir, false).await?;
            for user in store.list_users().await? {
                println!(
                    "{}\t{}\t{}",
                    user.login,
                    user.role,
                    if user.active { "active" } else { "disabled" }
                );
            }
            store.close().await;
        }
        _ => {
            return Err("usage: cuadb admin <bootstrap|reset-password|list-users> [--login NAME] [--password-stdin]".into());
        }
    }
    Ok(())
}
