//! Telemetry handle for plugins (delegates to the `telemetry` crate).

pub use telemetry::{Level, Telemetry};

/// Thin wrapper letting plugins emit telemetry.
#[derive(Clone, Debug)]
pub struct TelemetryApi {
    pub telemetry: Telemetry,
}

impl TelemetryApi {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    /// Emit an `INFO` event of the given kind.
    pub fn emit_info(&self, kind: &str, message: &str) {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind(kind.to_string())
            .category("plugin")
            .message(message.to_string())
            .emit();
    }

    /// Subscribe to the live event stream.
    pub fn subscribe(&self) -> telemetry::Subscription {
        self.telemetry.subscribe()
    }
}
