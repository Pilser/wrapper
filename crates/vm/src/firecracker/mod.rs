//! Firecracker REST API client over a Unix domain socket.

pub mod boot_source;
pub mod client;
pub mod config;
pub mod drive;
pub mod machine;
pub mod network;

pub use client::FirecrackerClient;
pub use config::{BootSourceConfig, DriveConfig, MachineConfig};
pub use network::{NetworkInterfaceConfig, gateway_for_ip, guest_mac_for_tenant, network_interface, tap_name_for_tenant};
