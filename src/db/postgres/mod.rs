//! PostgreSQL-only URL validation, connections and introspection safety.
pub mod manager;
pub mod url;

pub use manager::PgManager;
