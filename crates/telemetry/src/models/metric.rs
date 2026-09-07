//! Metric payload attached to `metric.*` events.

use serde::{Serialize, Deserialize};

/// A single metric reading (gauge / counter / histogram point).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPayload {
    pub name: String,
    /// `gauge`, `counter`, or `histogram`.
    pub kind: String,
    pub value: f64,
    pub unit: String,
    pub labels: std::collections::HashMap<String, String>,
}