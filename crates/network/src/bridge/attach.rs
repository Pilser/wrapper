//! Attach a device to a bridge.

use super::destroy::run_ip;

pub fn attach(bridge: &str, device: &str) -> anyhow::Result<()> {
    run_ip(&["link", "set", "dev", device, "master", bridge])
}

pub fn detach(device: &str) -> anyhow::Result<()> {
    run_ip(&["link", "set", "dev", device, "nomaster"])
}
