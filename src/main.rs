//! Launch the local HTTP server. No database connection is attempted in the shell.

use cuadb::{app, config::Config};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let address = (config.host, config.port);
    let listener = tokio::net::TcpListener::bind(address).await?;

    println!("CuaDB shell ready at http://{}:{}", config.host, config.port);
    axum::serve(listener, app::router()).await?;

    Ok(())
}
