//! `api` — thin plugin-facing facade re-exporting handles to the other
//! crates.
//!
//! Plugins talk to small wrapper structs here instead of importing every
//! crate directly. Each file stays small: a struct holding [`telemetry::Telemetry`]
//! plus constructors / delegating helpers or validated stubs.

pub mod admin;
pub mod auth;
pub mod context;
pub mod db;
pub mod events;
pub mod macros;
pub mod network;
pub mod plugin;
pub mod proxy;
pub mod supervisor;
pub mod telemetry;
pub mod vm;

pub use context::{ApiContext, ApiContextBuilder};

/// Version of the plugin API surface.
pub const API_VERSION: &str = "0.1.0";
