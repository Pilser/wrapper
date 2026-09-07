use core::Result;
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Record a supervisor heartbeat and acknowledge it.
pub async fn handle_heartbeat(
    tenant_id: &str,
    _payload: &Value,
    telemetry: &Telemetry,
) -> Result<Value> {
    crate::validate_tenant_id(tenant_id)?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.heartbeat.received")
        .category("supervisor")
        .message(format!("heartbeat received tenant {tenant_id}"))
        .emit();
    Ok(serde_json::json!({
        "ok": true,
        "tenant_id": tenant_id,
        "timestamp": core::now_rfc3339(),
    }))
}
