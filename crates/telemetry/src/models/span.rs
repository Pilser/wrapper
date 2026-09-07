//! Span payload tracking operation duration.

use serde::{Serialize, Deserialize};

/// Result of a completed span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    #[default]
    Success,
    Failed,
    Cancelled,
}

/// A timed operation history (span) attached to `span.completed` events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanPayload {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub span_name: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_ms: f64,
    pub status: SpanStatus,
    pub operation: String,
}