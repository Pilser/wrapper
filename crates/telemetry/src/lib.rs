//! `telemetry` — structured, disk-backed, broadcast event and metric pipeline.
//!
//! Every event conforms to the JSON schema in the architecture doc: id,
//! timestamp (RFC3339 nanos), level, type, category, source, optional
//! tenant/vm/session/trace/span ids, message, data, tags and typed payload.
//!
//! [`Telemetry`] is the single entry point: crates build it once at startup and
//! then call `telemetry.emit(event)` / `telemetry.record(...)`.

pub mod broadcast;
pub mod config;
pub mod event;
pub mod macros;
pub mod metrics;
pub mod models;
pub mod query;
pub mod writer;

pub use broadcast::{BroadcastFilter, BroadcastServer, Subscription};
pub use config::TelemetrySettings;
pub use event::{emit_event, EventBuilder, Level};
pub use metrics::{Counter, Gauge, Histogram, Metrics};
pub use models::{
    ErrorPayload, MetricPayload, RequestPayload, SpanPayload, SpanStatus, StatePayload,
};
pub use models::Event;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::writer::FileWriter;

/// The global telemetry handle shared across the process.
#[derive(Clone)]
pub struct Telemetry {
    pub config: Arc<TelemetrySettings>,
    pub metrics: Arc<Metrics>,
    enabled: bool,
    file_writer: Arc<RwLock<Option<FileWriter>>>,
    broadcast: Arc<BroadcastServer>,
}

impl std::fmt::Debug for Telemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Telemetry")
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl Telemetry {
    /// Build a telemetry instance from settings.
    pub fn init(settings: TelemetrySettings) -> Self {
        let enabled = settings.enabled;
        let file_writer = if settings.disk.enabled && output_writes_disk(&settings.output) {
            Arc::new(RwLock::new(Some(FileWriter::new(settings.disk.clone()))))
        } else {
            Arc::new(RwLock::new(None))
        };
        let broadcast = Arc::new(BroadcastServer::new(settings.broadcast.clone()));
        let metrics = Arc::new(Metrics::new());
        Self {
            config: Arc::new(settings),
            metrics,
            enabled,
            file_writer,
            broadcast,
        }
    }

    /// A fully disabled instance (used in tests / dry runs).
    pub fn noop() -> Self {
        let settings = TelemetrySettings::default();
        Self {
            enabled: false,
            config: Arc::new(settings),
            metrics: Arc::new(Metrics::new()),
            file_writer: Arc::new(RwLock::new(None)),
            broadcast: Arc::new(BroadcastServer::new(Default::default())),
        }
    }

    /// Whether this instance is active.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Start a fresh [`EventBuilder`] bound to this instance.
    pub fn builder(&self) -> EventBuilder {
        EventBuilder::new(self.clone())
    }

    /// Emit a ready-made event (file + broadcast + metrics).
    pub fn emit(&self, event: Event) {
        if !self.enabled {
            return;
        }
        let kind = event.kind.clone();
        let trace = event.trace_id.clone();
        self.broadcast.publish(event.clone());
        let file_writer = Arc::clone(&self.file_writer);
        let metrics = Arc::clone(&self.metrics);
        tokio::task::spawn(async move {
            {
                let mut guard = file_writer.write().await;
                if let Some(w) = guard.as_mut() {
                    let _ = w.write(&event).await;
                }
            }
            metrics.record_event(kind, trace);
        });
    }

    /// Record a metric value (emits a `metric.<kind>` event and aggregates).
    pub fn record(&self, name: &str, kind: &str, value: f64, unit: &str) {
        if !self.enabled {
            return;
        }
        let payload = MetricPayload {
            name: name.to_string(),
            kind: kind.to_string(),
            value,
            unit: unit.to_string(),
            labels: Default::default(),
        };
        let ev = EventBuilder::new(self.clone())
            .level(Level::Info)
            .kind(Event::metric_type(kind))
            .category("metrics")
            .message(format!("metric {name}={value:?} {unit}"))
            .metric(payload)
            .build();
        self.emit(ev);
        self.metrics.sample(name, value);
    }

    /// Subscribe to the live event stream (for dashboards / plugins).
    pub fn subscribe(&self) -> Subscription {
        self.broadcast.subscribe()
    }

    /// Flush buffered events to disk (awaited, used at shutdown).
    pub async fn flush(&self) {
        let mut guard = self.file_writer.write().await;
        if let Some(w) = guard.as_mut() {
            let _ = w.flush().await;
        }
    }
}

fn output_writes_disk(output: &str) -> bool {
    matches!(output, "disk" | "both")
}

/// Current RFC3339 timestamp with nanosecond precision.
pub fn now_string() -> String {
    core::now_rfc3339()
}