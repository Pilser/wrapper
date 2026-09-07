//! Tap configuration (addressing, bridge attachment).

use super::create::run_ip;

/// Attach a tap device to a Linux bridge.
pub fn attach_tap_to_bridge(tap: &str, bridge: &str) -> anyhow::Result<()> {
    run_ip(&["link", "set", "dev", tap, "master", bridge])
}

/// Configure an address for a tap device.
pub fn configure_tap(name: &str, ip: &str) -> anyhow::Result<()> {
    run_ip(&["addr", "add", &format!("{ip}/24"), "dev", name])
}
