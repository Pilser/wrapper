//! Event identity helpers.

use uuid::Uuid;

/// A unique event identifier, formatted `evt_<uuid>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventId(String);

impl EventId {
    /// Generate a fresh id.
    pub fn new() -> Self {
        Self(format!("evt_{}", Uuid::new_v4().simple()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Build a dotted event type for a metric, e.g. `metric.gauge`.
pub fn metric_type(kind: &str) -> String {
    format!("metric.{kind}")
}