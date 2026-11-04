//! Start the local-only server with bounded shutdown and no database connection.

use cuadb::{app, config::Config, state::AppState};
use std::{error::Error, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let state = AppState::from_config(&config)?;
    let listener = tokio::net::TcpListener::bind((config.host, config.port)).await?;
    let router = app::router_with_state(&config.limits, state.clone());
    state.start_probe();

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
            match tokio::time::timeout(Duration::from_secs(8), &mut server).await {
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
    if tokio::time::timeout(Duration::from_secs(2), state.close()).await.is_err() {
        eprintln!("level=warn event=database_pool_close code=deadline_exceeded");
    }
    Ok(())
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
