//! CuaDB application entry points.
//!
//! Keep the server router testable without binding a port or connecting to SQL.

pub mod app;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod profiles;
pub mod sql;
pub mod state;
pub mod web;
