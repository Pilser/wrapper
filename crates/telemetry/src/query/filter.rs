//! Event filter for re-querying stored events.

use serde::Deserialize;

/// Criteria used to filter a stored event collection.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventFilter {
    /// Dotted type, supports `vm.*` wildcards.
    pub event_type: Option<String>,
    pub level: Option<String>,
    pub tenant_id: Option<String>,
    pub vm_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub tags: Vec<String>,
    pub limit: Option<usize>,
}

impl EventFilter {
    /// Query-string parsing helper for `GET /admin/telemetry/events`.
    pub fn from_query(params: &std::collections::HashMap<String, String>) -> Self {
        EventFilter {
            event_type: params.get("type").cloned(),
            level: params.get("level").cloned(),
            tenant_id: params.get("tenant_id").cloned(),
            vm_id: params.get("vm_id").cloned(),
            from: params.get("from").cloned(),
            to: params.get("to").cloned(),
            tags: Vec::new(),
            limit: params.get("limit").and_then(|s| s.parse().ok()),
        }
    }
}