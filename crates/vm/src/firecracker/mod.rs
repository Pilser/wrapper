//! Firecracker REST API client over a Unix domain socket.

pub mod boot_source;
pub mod client;
pub mod config;
pub mod drive;
pub mod machine;

pub use client::FirecrackerClient;
pub use config::{BootSourceConfig, DriveConfig, MachineConfig};
