//! Emit custom telemetry events from plugins.

use telemetry::{Level, Telemetry};

/// Emit an `INFO` plugin event of the given kind.
pub fn emit_info(telemetry: &Telemetry, kind: &str, message: &str) {
    telemetry
        .builder()
        .level(Level::Info)
        .kind(kind.to_string())
        .category("plugin")
        .source("api::events::emit")
        .message(message.to_string())
        .tag("plugin")
        .emit();
}

/// Emit an `ERROR` plugin event of the given kind.
pub fn emit_error(telemetry: &Telemetry, kind: &str, message: &str) {
    telemetry
        .builder()
        .level(Level::Error)
        .kind(kind.to_string())
        .category("plugin")
        .source("api::events::emit")
        .message(message.to_string())
        .tag("plugin")
        .emit();
}
