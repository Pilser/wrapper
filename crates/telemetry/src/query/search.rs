//! Search stored daily JSON files for matching events.

use std::path::Path;

use serde_json::Value;

use super::filter::EventFilter;

/// Scan JSONL files in `dir` and return events matching `filter`.
pub fn search_events(dir: &Path, filter: &EventFilter) -> anyhow::Result<Vec<Value>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|e| e == "json").unwrap_or(false)
        })
        .collect();
    entries.sort();

    for path in entries {
        let lines = std::fs::read_to_string(&path)?;
        for line in lines.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(ev) = serde_json::from_str::<Value>(line) {
                if matches_filter(&ev, filter) {
                    out.push(ev);
                    if let Some(limit) = filter.limit {
                        if out.len() >= limit {
                            return Ok(out);
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

fn matches_filter(ev: &Value, filter: &EventFilter) -> bool {
    if let Some(kind) = &filter.event_type {
        let k = ev["type"].as_str().unwrap_or_default();
        if !pattern(kind, k) {
            return false;
        }
    }
    if let Some(lvl) = &filter.level {
        if ev["level"].as_str() != Some(lvl.as_str()) {
            return false;
        }
    }
    if let Some(tid) = &filter.tenant_id {
        if ev["tenant_id"].as_str() != Some(tid.as_str()) {
            return false;
        }
    }
    if let Some(from) = &filter.from {
        if ev["timestamp"].as_str().map(|t| t < from.as_str()).unwrap_or(true) {
            return false;
        }
    }
    if let Some(to) = &filter.to {
        if ev["timestamp"].as_str().map(|t| t > to.as_str()).unwrap_or(true) {
            return false;
        }
    }
    true
}

fn pattern(pattern: &str, kind: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        kind.starts_with(prefix)
    } else {
        pattern == kind
    }
}