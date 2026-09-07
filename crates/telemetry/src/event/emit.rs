//! Convenience emitters used by downstream crates.

use super::level::Level;
use super::types::EventId;
use crate::models::SpanPayload;
use crate::Telemetry;

/// Emit a simple event: `telemetry`, `level`, dotted `kind`, `message`.
pub fn emit_event(
    telemetry: &Telemetry,
    level: Level,
    kind: impl Into<String>,
    message: impl Into<String>,
) {
    telemetry.builder().level(level).kind(kind).message(message).emit();
}

/// A compile-time tracked timing helper that produces a Rust `Instant` plus a
/// name, ready to hand to [`crate::macros::span`].
pub struct SpanTimer {
    pub name: String,
    pub started_at: String,
    pub started: std::time::Instant,
}

impl SpanTimer {
    pub fn start(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            started_at: crate::now_string(),
            started: std::time::Instant::now(),
        }
    }

    /// Convert to a [`SpanPayload`] with the elapsed duration.
    pub fn finish(&self, status: crate::models::SpanStatus) -> SpanPayload {
        SpanPayload {
            span_id: EventId::new().to_string(),
            parent_span_id: None,
            span_name: self.name.clone(),
            started_at: self.started_at.clone(),
            ended_at: crate::now_string(),
            duration_ms: self.started.elapsed().as_secs_f64() * 1000.0,
            status,
            operation: self.name.clone(),
        }
    }
}

/// Create a fresh span timer by name.
pub fn span_timer(name: impl Into<String>) -> SpanTimer {
    SpanTimer::start(name)
}