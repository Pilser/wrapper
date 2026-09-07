//! Destroy a tap device.

use super::create::run_ip;

pub fn destroy_tap(name: &str) -> anyhow::Result<()> {
    run_ip(&["link", "del", "dev", name])
}
