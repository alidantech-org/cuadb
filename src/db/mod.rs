//! Database integration boundary (not yet implemented).
//!
//! Future adapters will discover schemas, tables and rows using engine-specific,
//! read-only queries. The current milestone intentionally opens no DB connections.

pub mod postgres;
