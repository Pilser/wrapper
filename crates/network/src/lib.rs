//! `network` — virtual networking for the Firecracker fleet: tap devices, the
//! tenant IP pool, vsock, bridge, NAT and firewall rule management.

pub mod bridge;
pub mod firewall;
pub mod ip;
pub mod monitor;
pub mod nat;
pub mod tap;
pub mod vsock;

pub use firewall::FirewallManager;
pub use ip::IpPool;
pub use monitor::{Bandwidth, TrafficMonitor};
pub use tap::{TapCreateOptions, create_tap, destroy_tap, list_taps};
pub use vsock::VsockConfig;

use core::NetworkConfig;
use telemetry::Telemetry;

/// Top-level facade tying the virtual-networking subcomponents together.
#[derive(Clone)]
pub struct NetworkManager {
    telemetry: Telemetry,
    pub ip_pool: IpPool,
    pub firewall: FirewallManager,
    pub monitor: TrafficMonitor,
}

impl NetworkManager {
    pub fn new(cfg: NetworkConfig) -> anyhow::Result<Self> {
        Self::with_telemetry(cfg, Telemetry::noop())
    }

    pub fn with_telemetry(cfg: NetworkConfig, telemetry: Telemetry) -> anyhow::Result<Self> {
        Ok(Self {
            ip_pool: IpPool::with_telemetry(cfg, telemetry.clone()),
            firewall: FirewallManager::with_telemetry(telemetry.clone()),
            monitor: TrafficMonitor::with_telemetry(telemetry.clone()),
            telemetry,
        })
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }
}
