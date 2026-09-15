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
        // Asked by in-VM supervisors before tools/list: which dotted ops
        // may this tenant list and call.
        "tools.granted" | "tools_granted" => {
            handle_tools_granted(envelope, db, telemetry).await
        }
        // Asked by in-VM supervisors at boot (and on repeated program
        // crashes): the launcher scripts that start this tenant's programs.
        // No launchers are baked into the image; without these the guest
        // runs nothing. Scope precedence per program: tenant > category >
        // global. Missing program => supervisor keeps demanding.
        "launchers.get" | "launchers_get" => {
            handle_launchers_get(envelope, db, telemetry).await
        }
        // Supervisor binary demand: version manifest + 256KB chunk fetch.
        // Served from the wrapper `[binaries]` host folder.
        "binaries.manifest" | "binaries_manifest" => {
            handle_binaries_manifest(envelope, db, telemetry).await
        }
        "binaries.get" | "binaries_get" => {
            handle_binaries_get(envelope, db, telemetry).await
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

/// Translate a supervisor dotted op (`portfolio.show`) to the short verb the
/// portfolio handler matches on.
fn short_verb(dotted: &str) -> Option<&'static str> {
    Some(match dotted {
        "portfolio.show" => "show",
        "portfolio.update" => "update",
        "portfolio.create_goal" => "create_goal",
        "portfolio.update_goal" => "update_goal",
        "portfolio.list_goals" => "list_goals",
        "portfolio.get_goal" => "get_goal",
        "portfolio.add_milestone" => "add_milestone",
        "portfolio.list_milestones" => "list_milestones",
        "portfolio.update_milestone" => "update_milestone",
        "portfolio.create_task" => "create_task",
        "portfolio.update_task" => "update_task",
        "portfolio.list_tasks" => "list_tasks",
        "portfolio.get_task" => "get_task",
        "portfolio.submit_attempt" => "submit_attempt",
        "portfolio.resolve_attempt" => "resolve_attempt",
        "portfolio.list_attempts" => "list_attempts",
        "portfolio.get_attempt" => "get_attempt",
        "portfolio.record_skill" => "record_skill",
        "portfolio.update_skill" => "update_skill",
        "portfolio.list_skills" => "list_skills",
        "portfolio.get_skill" => "get_skill",
        "portfolio.award_achievement" => "award_achievement",
        "portfolio.list_achievements" => "list_achievements",
        "portfolio.get_achievement" => "get_achievement",
        "portfolio.generate_report" => "generate_report",
        "portfolio.list_reports" => "list_reports",
        "portfolio.get_report" => "get_report",
        "portfolio.list_timeline" => "list_timeline",
        "portfolio.get_timeline" => "get_timeline",
        "portfolio.add_timeline" => "add_timeline",
        _ => return None,
    })
}

/// Category gate: tenants whose category has explicit tool rows may only
/// call those dotted ops. No rows = unrestricted (backward compatible).
/// Returns the granted set (empty vec = unrestricted) for `tools.granted`.
pub async fn category_grants(
    db: &db::Db,
    tenant_id: &str,
) -> Result<Vec<String>> {
    let gconn = db
        .global()
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let tenant = db::global::tenants::get_by_id(&gconn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound(format!("tenant {tenant_id}")))?;
    db::global::category_tools::list(&gconn, &tenant.category_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))
}

async fn enforce_category_grant(
    db: &db::Db,
    tenant_id: &str,
    op: &str,
) -> Result<()> {
    let grants = category_grants(db, tenant_id).await?;
    if !grants.is_empty() && !grants.iter().any(|g| g == op) {
        // NOTE: InvalidInput, not Unauthorized — the supervisor proxy maps
        // any "unauthorized"/"forbidden" text to a generic token error,
        // which would hide which tool was denied and why.
        return Err(Error::InvalidInput(format!(
            "tool {op} is not granted to this tenant category"
        )));
    }
    Ok(())
}

/// Host binaries dir for the TCP channel (no config reaches this layer):
/// `$WRAPPER_BINARIES_DIR`, else `../binaries` (deployment layout: the
/// wrapper runs with cwd `deployment/wrapper`), else `./binaries`.
fn binaries_host_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("WRAPPER_BINARIES_DIR") {
        if !d.trim().is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    for cand in ["../binaries", "./binaries"] {
        let p = std::path::PathBuf::from(cand);
        if p.exists() {
            return p;
        }
    }
    std::path::PathBuf::from("../binaries")
}

/// Answer `binaries.manifest` with `{files:[{path,sha256,size}]}`.
pub async fn handle_binaries_manifest(
    envelope: &Envelope,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    let _ = (envelope, db, telemetry);
    let files = shared::binaries::manifest(&binaries_host_dir());
    Ok(serde_json::json!({ "files": files }))
}

/// Answer `binaries.get {path, offset?, len?}` with `{data_base64, sha256,
/// size}`. 256KB chunks keep NDJSON frames small; the supervisor verifies
/// the full-file sha after assembly.
pub async fn handle_binaries_get(
    envelope: &Envelope,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    let _ = (db, telemetry);
    let payload = &envelope.payload;
    let rel = payload
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !shared::binaries::validate_rel(rel) {
        return Err(Error::InvalidInput("bad path".to_string()));
    }
    let full = std::fs::read(binaries_host_dir().join(rel))
        .map_err(|_| Error::NotFound(format!("binary not found: {rel}")))?;
    let offset = payload
        .get("offset")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let len = payload
        .get("len")
        .and_then(|v| v.as_u64())
        .unwrap_or(262_144) as usize;
    let len = len.clamp(1, 1_048_576);
    if offset > full.len() {
        return Err(Error::InvalidInput("offset past end".to_string()));
    }
    let end = std::cmp::min(offset + len, full.len());
    Ok(serde_json::json!({
        "data_base64": base64_encode(&full[offset..end]),
        "sha256": shared::binaries::sha256_bytes(&full),
        "size": full.len(),
    }))
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let mut n: u32 = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            n |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            n |= chunk[2] as u32;
        }
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Answer `launchers.get` with `{scripts: {zw: {content, source}, engram:
/// {...}}}` for this tenant. Scope precedence per program: tenant >
/// category > global. A program with no row anywhere is reported with
/// `content: null` — the supervisor keeps demanding until it appears.
pub async fn handle_launchers_get(
    envelope: &Envelope,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    let _ = telemetry;
    let gconn = db
        .global()
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let tenant_id = envelope.tenant_id.as_str();
    let category_id = db::global::tenants::get_by_id(&gconn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .map(|t| t.category_id)
        .unwrap_or_default();
    let mut scripts = serde_json::Map::new();
    for name in db::global::launchers::NAMES {
        let (content, source) =
            db::global::launchers::effective(&gconn, tenant_id, &category_id, name)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
        scripts.insert(
            name.to_string(),
            serde_json::json!({ "content": content, "source": source }),
        );
    }
    Ok(serde_json::json!({ "scripts": scripts }))
}

/// Answer `tools.granted` with `{tools, restricted}` for this tenant.
/// `restricted=false` (no rows) means list everything; `restricted=true`
/// means exactly `tools` (possibly empty = deny all).
pub async fn handle_tools_granted(
    envelope: &Envelope,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    let _ = telemetry;
    let grants = category_grants(db, &envelope.tenant_id).await?;
    if grants.is_empty() {
        return Ok(serde_json::json!({ "tools": super::tools::tool_names(), "restricted": false }));
    }
    Ok(serde_json::json!({ "tools": grants, "restricted": true }))
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
    enforce_category_grant(db, &envelope.tenant_id, op).await?;
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
