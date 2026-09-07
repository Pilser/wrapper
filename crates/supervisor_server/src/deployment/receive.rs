use shared::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

use super::package::parse_package;
use super::verify::{emit_verified, verify_package};

/// Handle a supervisor `deployment.ack` / `deployment.received` envelope.
/// The payload carries the [`shared::DeploymentManifest`] being acknowledged.
pub async fn handle_deployment_receive(
    tenant_id: &str,
    payload: &Value,
    telemetry: &Telemetry,
) -> Result<Value> {
    crate::validate_tenant_id(tenant_id)?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.deployment.sent")
        .category("supervisor")
        .message(format!("deployment received tenant {tenant_id}"))
        .emit();
    let manifest = parse_package(payload)?;
    if manifest.tenant_id != tenant_id {
        return Err(Error::Unauthorized(
            "deployment tenant mismatch".to_string(),
        ));
    }
    verify_package(&manifest)?;
    emit_verified(telemetry, tenant_id, &manifest.category);
    Ok(serde_json::json!({
        "ok": true,
        "tenant_id": tenant_id,
        "category": manifest.category,
        "artifacts": manifest.artifacts.len(),
    }))
}
