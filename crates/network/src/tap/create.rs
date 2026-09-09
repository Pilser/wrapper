//! Create a tap device.

use shared::TapDevice;

use super::configure::attach_tap_to_bridge;

/// Options for creating a tap device.
#[derive(Debug, Clone, Default)]
pub struct TapCreateOptions {
    pub name: String,
    pub ip: String,
}

/// Create a tap device named `opts.name` and attach it to a bridge.
/// Idempotent: when the device already exists (previous run, rebooted VM) it
/// is reused instead of failing.
pub fn create_tap(opts: TapCreateOptions) -> anyhow::Result<TapDevice> {
    if !device_exists(&opts.name) {
        run_ip(&["tuntap", "add", "dev", &opts.name, "mode", "tap"])?;
    }
    attach_tap_to_bridge(&opts.name, "br0")?;
    Ok(TapDevice {
        name: opts.name,
        ip: opts.ip,
    })
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

pub(crate) fn run_ip(args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new("ip").args(args).status()?;
    if !status.success() {
        anyhow::bail!("ip {:?} failed ({status})", args);
    }
    Ok(())
}
