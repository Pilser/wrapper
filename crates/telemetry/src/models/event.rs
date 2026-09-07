//! The `Event` — the canonical unit emitted, stored and broadcast.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::event::{EventId, Level};
use crate::models::{
    ErrorPayload, MetricPayload, RequestPayload, SpanPayload, StatePayload,
};

/// A single telemetry event, matching the documented JSON schema.
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: String,
    pub timestamp: String,
    pub level: Level,
    #[serde(rename = "type")]
    pub kind: String,
    pub category: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub data: Map<String, Value>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric: Option<MetricPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<StatePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<SpanPayload>,
}

impl Event {
    /// A fresh event shell with an id and timestamp filled in.
    pub fn new() -> Self {
        Self {
            id: EventId::new().to_string(),
            timestamp: crate::now_string(),
            level: Level::Info,
            kind: String::new(),
            category: String::new(),
            source: String::new(),
            tenant_id: None,
            vm_id: None,
            session_id: None,
            trace_id: None,
            span_id: None,
            message: None,
            data: Map::new(),
            tags: Vec::new(),
            tenant_category: None,
            error: None,
            metric: None,
            request: None,
            state: None,
            span: None,
        }
    }

    /// Dotted type for a metric event, e.g. `metric.gauge`.
    pub fn metric_type(kind: &str) -> String {
        format!("metric.{kind}")
    }
}

impl Default for Event {
    fn default() -> Self {
        Self::new()
    }
}