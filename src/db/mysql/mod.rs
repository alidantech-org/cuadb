//! MySQL/MariaDB adapter implementation.
pub mod catalog;
pub mod manager;
pub mod url;

pub use catalog::MySqlCatalog;
pub use manager::MySqlManager;
