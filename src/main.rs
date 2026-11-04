//! Start the local-only server with bounded shutdown and no database connection.

use cuadb::{app, config::Config, state::AppState};
use std::{error::Error, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    if std::env::args().nth(1).as_deref() == Some("admin") {
        return admin_command(&config).await;
    }
    let state = AppState::from_config(&config)?
        .with_auth_from_config(&config)
        .await?;
    if state.auth().is_none() {
        eprintln!(
            "level=warn event=auth_not_configured message=\"no administrator exists: the local read-only explorer is open to anyone who can reach this port; run `cuadb admin bootstrap` before exposing it\""
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
