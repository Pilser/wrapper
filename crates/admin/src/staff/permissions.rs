//! Staff permissions (stubs emitting telemetry).

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// Grant a permission to a staff member.
pub async fn grant_permission(
    telemetry: &Telemetry,
    staff_id: &str,
    permission: &str,
) -> Result<()> {
    if staff_id.trim().is_empty() || permission.trim().is_empty() {
        return Err(Error::InvalidInput(
            "staff_id and permission must not be empty".into(),
        ));
    }
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.staff.updated")
        .category("admin")
        .source("admin::staff::permissions")
        .message(format!("permission {permission} granted to {staff_id}"))
        .data("staff_id", staff_id.to_string())
        .tag("admin")
        .tag("staff")
        .emit();
    Ok(())
}

/// Revoke a permission from a staff member.
pub async fn revoke_permission(
    telemetry: &Telemetry,
    staff_id: &str,
    permission: &str,
) -> Result<()> {
    if staff_id.trim().is_empty() || permission.trim().is_empty() {
        return Err(Error::InvalidInput(
            "staff_id and permission must not be empty".into(),
        ));
    }
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.staff.updated")
        .category("admin")
        .source("admin::staff::permissions")
        .message(format!("permission {permission} revoked from {staff_id}"))
        .data("staff_id", staff_id.to_string())
        .tag("admin")
        .tag("staff")
        .emit();
    Ok(())
}
