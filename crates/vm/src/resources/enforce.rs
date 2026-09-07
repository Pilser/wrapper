//! Enforce resource limits (throttle / kill).

use telemetry::{Level, Telemetry};

use super::limits::ResourceLimits;
use super::monitor::ResourceMonitor;

/// Take action when a VM exceeds its limits.
pub fn enforce(telemetry: &Telemetry, monitor: &ResourceMonitor, vm_id: &str, limits: &ResourceLimits) -> anyhow::Result<()> {
    if monitor.exceeds(vm_id, limits) {
        telemetry.builder().level(Level::Warn).kind("vm.resource.threshold_exceeded")
            .category("vm").message(format!("vm {vm_id} exceeded resource limits"))
            .vm_id(Some(vm_id.to_string())).emit();
    }
    Ok(())
}
