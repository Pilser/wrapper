use core::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

use crate::protocol::message::Envelope;

/// Route an incoming envelope to the right subsystem.
pub async fn dispatch(
    envelope: &Envelope,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    envelope.validate()?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.tool.called")
        .category("supervisor")
        .message(format!(
            "received {} for tenant {}",
            envelope.msg_type, envelope.tenant_id
        ))
        .emit();
    match envelope.msg_type.as_str() {
        "tool.call" | "mcp.request" | "tool_call" => {
            handle_tool_call(envelope, db, telemetry).await
        }
        "heartbeat" => {
            crate::heartbeat::receive::handle_heartbeat(
                &envelope.tenant_id,
                &envelope.payload,
                telemetry,
            )
            .await
        }
        "deployment.ack" | "deployment.received" => {
            crate::deployment::receive::handle_deployment_receive(
                &envelope.tenant_id,
                &envelope.payload,
                telemetry,
            )
            .await
        }
        "telemetry.event" | "telemetry_event" => {
            super::telemetry_ingest::handle_telemetry_event(
                &envelope.tenant_id,
                &envelope.payload,
                telemetry,
            )
            .await
        }
        other => Err(Error::InvalidInput(format!(
            "unknown message type {other}"
        ))),
    }
}

pub async fn handle_tool_call(
    envelope: &Envelope,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    let tool = envelope
        .payload
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let tool = if tool.is_empty() && envelope.msg_type != "tool.call" {
        envelope.msg_type.as_str()
    } else {
        tool
    };
    let claimed = envelope
        .payload
        .get("tenant_id")
        .and_then(|v| v.as_str());
    super::permissions::check_tenant_match(&envelope.tenant_id, claimed)?;
    super::permissions::can_call(&envelope.tenant_id, tool)?;
    crate::tools::dispatch_tool(tool, &envelope.tenant_id, &envelope.payload, db, telemetry)
        .await
}

pub fn success_response(req: &Envelope, data: Value) -> Envelope {
    req.respond("response", data)
}

pub fn error_response(req: &Envelope, err: &Error) -> Envelope {
    req.respond("error", serde_json::json!({"error": err.to_string()}))
}
