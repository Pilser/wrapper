use shared::{Error, Result};
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
        // Supervisor ProxyClient dialect: {type: db_read/db_write,
        // payload: {op: "memory.save", data: {...}}}. `op` is the dotted
        // tool name dispatch_tool already routes on.
        "db_read" | "db_write" => {
            handle_proxy_op(envelope, db, telemetry).await
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

/// Translate a supervisor dotted op (`memory.save`) to the short verb the
/// family handlers match on (`save`), with family-specific aliases.
fn short_verb(dotted: &str) -> Option<&'static str> {
    Some(match dotted {
        "memory.save" => "save",
        "memory.get" => "get",
        "memory.list" => "list",
        "memory.search" => "search",
        "memory.delete" => "delete",
        "profile.get" => "get",
        "profile.update" => "update",
        "history.log" => "append",
        "history.get" => "get",
        "projects.create" => "create",
        "projects.update" => "update",
        "projects.list" => "list",
        "projects.get" => "get",
        "skills.add" => "add",
        "skills.update" => "update",
        "skills.list" => "list",
        "skills.get" => "get",
        "reports.get" => "get",
        "reports.list" => "list",
        "configs.get" => "get",
        "configs.set" => "set",
        "tenant.get_tenant_info" | "tenant.info" => "info",
        "tenant.get_category_info" | "tenant.category" => "category",
        _ => return None,
    })
}

pub async fn handle_proxy_op(
    envelope: &Envelope,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    let op = envelope
        .payload
        .get("op")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let verb = short_verb(op).ok_or_else(|| {
        Error::InvalidInput(format!("unknown supervisor op {op:?}"))
    })?;
    let claimed = envelope
        .payload
        .get("tenant_id")
        .and_then(|v| v.as_str());
    super::permissions::check_tenant_match(&envelope.tenant_id, claimed)?;
    super::permissions::can_call(&envelope.tenant_id, op)?;
    // Hoist data.* up so family handlers read args flat or nested alike,
    // and set the short verb they match on.
    let mut payload = envelope.payload.clone();
    if let Some(obj) = payload.as_object_mut() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()).cloned() {
            for (k, v) in data {
                obj.entry(k).or_insert(v);
            }
        }
        obj.insert("op".to_string(), Value::String(verb.to_string()));
    }
    crate::tools::dispatch_tool(op, &envelope.tenant_id, &payload, db, telemetry)
        .await
}

pub fn success_response(req: &Envelope, data: Value) -> Envelope {
    req.respond("response", data)
}

pub fn error_response(req: &Envelope, err: &Error) -> Envelope {
    req.respond("error", serde_json::json!({"error": err.to_string()}))
}
