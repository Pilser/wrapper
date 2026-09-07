//! Staff audit trail (stubs emitting telemetry).

use shared::{Error, Result};
use telemetry::{Level, Telemetry};

/// One auditable staff action.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
}

/// Record an audit entry via telemetry.
pub async fn log_audit(telemetry: &Telemetry, entry: AuditEntry) -> Result<()> {
    if entry.actor.trim().is_empty() || entry.action.trim().is_empty() {
        return Err(Error::InvalidInput(
            "actor and action must not be empty".into(),
        ));
    }
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.staff.updated")
        .category("admin")
        .source("admin::staff::audit")
        .message(format!("audit: {} by {}", entry.action, entry.actor))
        .data("actor", entry.actor.clone())
        .data("action", entry.action.clone())
        .tag("admin")
        .tag("staff")
        .tag("audit")
        .emit();
    Ok(())
}
