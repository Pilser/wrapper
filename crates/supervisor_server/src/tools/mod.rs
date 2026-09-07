pub mod configs;
pub mod history;
pub mod memory;
pub mod profile;
pub mod projects;
pub mod reports;
pub mod skills;
pub mod tenant;

pub use configs::handle_configs_call;
pub use history::handle_history_call;
pub use memory::handle_memory_call;
pub use profile::handle_profile_call;
pub use projects::handle_projects_call;
pub use reports::handle_reports_call;
pub use skills::handle_skills_call;
pub use tenant::handle_tenant_call;

use core::{Error, Result};
use serde_json::Value;
use telemetry::Telemetry;

/// Look up a string argument, checking the transport wrapper
/// `payload["data"][key]` first and then the flat `payload[key]`.
/// (The reference `memory` handler reads flat top-level args; this accepts
/// both shapes.)
pub fn str_arg<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload
        .get("data")
        .and_then(|d| d.get(key))
        .or_else(|| payload.get(key))
        .and_then(|v| v.as_str())
}

/// Look up an optional integer argument (e.g. `limit`, `duration_seconds`).
pub fn int_arg(payload: &Value, key: &str) -> Option<i64> {
    payload
        .get("data")
        .and_then(|d| d.get(key))
        .or_else(|| payload.get(key))
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
}

/// Look up an optional float argument (e.g. `importance`, `confidence`).
pub fn float_arg(payload: &Value, key: &str) -> Option<f64> {
    payload
        .get("data")
        .and_then(|d| d.get(key))
        .or_else(|| payload.get(key))
        .and_then(|v| v.as_f64())
}

/// Route a dotted tool name (`memory.list`, `profile.get`, ...) to its handler.
pub async fn dispatch_tool(
    tool: &str,
    tenant_id: &str,
    payload: &Value,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    match tool {
        t if t.starts_with("memory") => memory::handle_memory_call(tenant_id, payload, db, telemetry).await,
        t if t.starts_with("profile") => profile::handle_profile_call(tenant_id, payload, db, telemetry).await,
        t if t.starts_with("history") => history::handle_history_call(tenant_id, payload, db, telemetry).await,
        t if t.starts_with("projects") => projects::handle_projects_call(tenant_id, payload, db, telemetry).await,
        t if t.starts_with("skills") => skills::handle_skills_call(tenant_id, payload, db, telemetry).await,
        t if t.starts_with("reports") => reports::handle_reports_call(tenant_id, payload, db, telemetry).await,
        t if t.starts_with("configs") => configs::handle_configs_call(tenant_id, payload, db, telemetry).await,
        t if t.starts_with("tenant") => tenant::handle_tenant_call(tenant_id, payload, db, telemetry).await,
        _ => Err(Error::NotFound(format!("unknown tool {tool}"))),
    }
}
