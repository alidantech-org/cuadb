//! Shared server-owned state. No mutable global connection registry.

use std::sync::Arc;

use crate::{
    config::Config,
    db::{
        postgres::{PgManager, url::UrlError},
        types::{ConnectionSnapshot, ConnectionStatus},
    },
};

#[derive(Clone)]
pub struct AppState {
    postgres: Option<Arc<PgManager>>,
}

impl AppState {
    pub fn from_config(config: &Config) -> Result<Self, UrlError> {
        let postgres = config
            .database_url()
            .map(|secret| PgManager::new(secret.expose(), &config.limits))
            .transpose()?
            .map(Arc::new);
        Ok(Self { postgres })
    }

    pub fn disconnected() -> Self {
        Self { postgres: None }
    }

    pub async fn connection_snapshot(&self) -> ConnectionSnapshot {
        match &self.postgres {
            Some(manager) => manager.snapshot().await,
            None => ConnectionSnapshot::disconnected(),
        }
    }

    /// Optional startup probe: errors change status, never crash the UI.
    pub fn start_probe(&self) {
        if let Some(manager) = &self.postgres {
            let manager = Arc::clone(manager);
            let _task = tokio::spawn(async move {
                let _ = manager.snapshot().await;
            });
        }
    }

    pub async fn close(&self) {
        if let Some(manager) = &self.postgres {
            manager.close().await;
        }
    }

    pub fn is_configured(&self) -> bool {
        self.postgres.is_some()
    }
}

impl ConnectionSnapshot {
    /// Static user-facing words, never operator credentials or raw driver text.
    pub fn label(self) -> &'static str {
        match self.status {
            ConnectionStatus::Disconnected => "Not configured",
            ConnectionStatus::Connecting => "Connecting",
            ConnectionStatus::Ready => "Connected",
            ConnectionStatus::Unavailable => "Unavailable",
            ConnectionStatus::AuthenticationFailed => "Authentication failed",
            ConnectionStatus::PermissionDenied => "Insufficient permissions",
        }
    }

    pub fn detail(self) -> &'static str {
        match self.status {
            ConnectionStatus::Disconnected => "Set CUADB_DATABASE_URL on the server.",
            ConnectionStatus::Connecting => "Checking the configured PostgreSQL connection.",
            ConnectionStatus::Ready => "PostgreSQL connection and read-only session verified.",
            ConnectionStatus::Unavailable => "Cannot reach the configured PostgreSQL database.",
            ConnectionStatus::AuthenticationFailed => "Check the configured viewer credentials.",
            ConnectionStatus::PermissionDenied => "The viewer role lacks required access.",
        }
    }

    pub fn is_ready(self) -> bool {
        self.status == ConnectionStatus::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_url_remains_disconnected_without_network() {
        let config = Config::from_values(&[]).unwrap();
        let state = AppState::from_config(&config).unwrap();
        assert!(!state.is_configured());
        assert_eq!(
            state.connection_snapshot().await.status,
            ConnectionStatus::Disconnected
        );
        state.close().await;
    }

    #[test]
    fn malformed_connection_url_fails_at_startup_without_echoing_password() {
        let config = Config::from_values(&[(
            "CUADB_DATABASE_URL",
            "postgresql://viewer:secret@db/db?options=-c+bad",
        )])
        .unwrap();
        let outcome = AppState::from_config(&config).err().unwrap();
        assert!(!format!("{outcome:?}").contains("secret"));
    }
}
