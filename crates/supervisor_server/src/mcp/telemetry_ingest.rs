use shared::Result;
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle an inbound `"telemetry.event"` envelope.
///
/// The payload is the supervisor's telemetry event JSON — either the event
/// object itself or the transport wrapper `{"op", "data", "token"}` with the
/// event object under `data`. The event is persisted through the wrapper
/// [`Telemetry`] pipeline (file + broadcast + metrics, subject to the
/// wrapper's telemetry settings) and an ack payload is returned to the
/// supervisor.
pub async fn handle_telemetry_event(
    tenant_id: &str,
    payload: &Value,
    telemetry: &Telemetry,
) -> Result<Value> {
    crate::validate_tenant_id(tenant_id)?;
    let event = payload
        .get("data")
        .filter(|v| v.is_object())
        .unwrap_or(payload);
    let kind = event
        .get("type")
        .and_then(|v| v.as_str())
        .or_else(|| event.get("kind").and_then(|v| v.as_str()))
        .unwrap_or("supervisor.event");
    let level = event
        .get("level")
        .and_then(|v| v.as_str())
        .map(Level::from_str)
        .unwrap_or(Level::Info);
    let category = event
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("supervisor");
    let message = event
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("supervisor telemetry event");
    let mut builder = telemetry
        .builder()
        .level(level)
        .kind(kind)
        .category(category)
        .source(
            event
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("supervisor::telemetry"),
        )
        .tenant_id(Some(tenant_id.to_string()))
        .message(message)
        .data("event", event.clone());
    for key in ["vm_id", "session_id", "trace_id", "span_id"] {
        if let Some(v) = event.get(key).and_then(|v| v.as_str()) {
            builder = match key {
                "vm_id" => builder.vm_id(Some(v.to_string())),
                "session_id" => builder.session_id(Some(v.to_string())),
                "trace_id" => builder.trace_id(Some(v.to_string())),
                _ => builder.span_id(Some(v.to_string())),
            };
        }
    }
    if let Some(tags) = event.get("tags").and_then(|v| v.as_array()) {
        for tag in tags.iter().filter_map(|t| t.as_str()) {
            builder = builder.tag(tag);
        }
    }
    builder.emit();
    Ok(serde_json::json!({
        "ok": true,
        "tenant_id": tenant_id,
        "type": kind,
    }))
}
