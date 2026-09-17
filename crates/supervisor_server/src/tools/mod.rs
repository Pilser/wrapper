pub mod hub;
pub mod portfolio;

pub use portfolio::handle_portfolio_call;

use shared::{Error, Result};
use serde_json::Value;
use telemetry::Telemetry;

/// Look up a string argument, checking the transport wrapper
/// `payload["data"][key]` first and then the flat `payload[key]`.
pub fn str_arg<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload
        .get("data")
        .and_then(|d| d.get(key))
        .or_else(|| payload.get(key))
        .and_then(|v| v.as_str())
}

/// Look up an optional integer argument (e.g. `limit`, `id`).
pub fn int_arg(payload: &Value, key: &str) -> Option<i64> {
    payload
        .get("data")
        .and_then(|d| d.get(key))
        .or_else(|| payload.get(key))
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
}

/// Look up an optional float argument (e.g. `confidence`).
pub fn float_arg(payload: &Value, key: &str) -> Option<f64> {
    payload
        .get("data")
        .and_then(|d| d.get(key))
        .or_else(|| payload.get(key))
        .and_then(|v| v.as_f64())
}

/// Route a dotted tool name to its handler — only portfolio.* (hard cut).
pub async fn dispatch_tool(
    tool: &str,
    tenant_id: &str,
    payload: &Value,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    match tool {
        t if t.starts_with("portfolio") => portfolio::handle_portfolio_call(tenant_id, payload, db, telemetry).await,
        _ => Err(Error::NotFound(format!("unknown tool {tool}"))),
    }
}
