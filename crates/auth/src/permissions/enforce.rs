//! Enforce permissions (deny with context when unauthorized).

use telemetry::{Level, Telemetry};

use super::check::check_permission;
use super::roles::Role;

pub fn enforce(telemetry: &Telemetry, role: Role, resource: &str, action: &str) -> anyhow::Result<()> {
    if !check_permission(role, resource, action) {
        telemetry.builder().level(Level::Warn).kind("auth.permission.denied")
            .category("auth").message(format!("role {:?} denied {action} on {resource}", role)).emit();
        anyhow::bail!("permission denied: {action} on {resource}");
    }
    Ok(())
}
