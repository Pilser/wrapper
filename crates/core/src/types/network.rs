//! Networking primitives managed by the wrapper.

use serde::{Deserialize, Serialize};

/// Overall network configuration derived from `[network]` config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    pub ip_range: String,
    pub gateway: String,
    pub netmask: String,
    pub max_tenants: usize,
}

/// A single IP handed out to a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpAllocation {
    pub ip_address: String,
    pub tap_device: String,
    pub tenant_id: Option<String>,
    pub allocated_at: String,
}

/// A created tap device bound to a bridge/network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapDevice {
    pub name: String,
    pub ip: String,
}

/// vsock configuration for host<->guest control channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VsockConfig {
    pub cid: u32,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallAction {
    Allow,
    Deny,
}

/// A firewall rule restricting per-VM traffic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    pub id: String,
    pub action: FirewallAction,
    pub protocol: String,
    pub src: Option<String>,
    pub dst: Option<String>,
    pub port: Option<u16>,
}