//! PostgreSQL-only URL validation, connections and introspection safety.
pub mod catalog;
pub mod manager;
pub mod url;

pub use catalog::PgCatalog;
pub use manager::PgManager;
