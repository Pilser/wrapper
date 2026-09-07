//! Export event search results as JSON or CSV.

use serde_json::Value;

/// Render events as a JSON array.
pub fn as_json(events: &[Value]) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(events)?)
}

/// Render events as CSV with a fixed column set.
pub fn as_csv(events: &[Value]) -> String {
    let columns = [
        "timestamp",
        "level",
        "type",
        "category",
        "tenant_id",
        "vm_id",
        "message",
    ];
    let mut out = String::from(columns.join(","));
    out.push('\n');
    for ev in events {
        let row: Vec<String> = columns
            .iter()
            .map(|c| ev[c].as_str().unwrap_or("").to_string())
            .collect();
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}