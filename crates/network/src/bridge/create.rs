//! Create a network bridge.

use super::destroy::run_ip;

pub fn create_bridge(name: &str) -> anyhow::Result<()> {
    run_ip(&["link", "add", "name", name, "type", "bridge"])
}

pub fn set_bridge_up(name: &str) -> anyhow::Result<()> {
    run_ip(&["link", "set", "dev", name, "up"])
}
