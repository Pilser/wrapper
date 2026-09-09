//! Create a network bridge.

use super::destroy::run_ip;

pub fn create_bridge(name: &str) -> anyhow::Result<()> {
    run_ip(&["link", "add", "name", name, "type", "bridge"])
}

pub fn set_bridge_up(name: &str) -> anyhow::Result<()> {
    run_ip(&["link", "set", "dev", name, "up"])
}

fn device_exists(name: &str) -> bool {
    std::process::Command::new("ip")
        .args(["link", "show", "dev", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Idempotent host bridge setup: create `br0`-style bridge if missing, assign
/// the gateway address (tolerated when already assigned), and bring it up.
/// Must run as root; returns an error otherwise.
pub fn ensure_bridge(name: &str, gateway_ip: &str) -> anyhow::Result<()> {
    if !device_exists(name) {
        create_bridge(name)?;
    }
    // Tolerate "already assigned" — the bridge survives wrapper restarts.
    let _ = std::process::Command::new("ip")
        .args([
            "addr",
            "add",
            &format!("{gateway_ip}/24"),
            "dev",
            name,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    set_bridge_up(name)?;
    Ok(())
}
