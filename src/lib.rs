//! CuaDB application entry points.
//!
//! Keep the server router testable without binding a port or connecting to SQL.

pub mod app;
pub mod assets;
pub mod auth;
pub mod config;
pub mod connections;
pub mod db;
pub mod error;
pub mod profiles;
pub mod secrets;
pub mod sql;
pub mod state;
pub mod web;
