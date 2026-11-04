//! Read-only database abstraction; engine-specific code lives below postgres/.
pub mod adapter;
pub mod postgres;
pub mod types;
