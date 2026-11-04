//! CuaDB application entry points.
//!
//! The library exposes the router separately from the executable so HTTP tests
//! can run without opening a listening socket or connecting to a database.

pub mod app;
pub mod config;
pub mod db;
pub mod web;
