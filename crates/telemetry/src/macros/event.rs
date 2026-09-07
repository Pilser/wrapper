//! `telemetry_event!` — inline event emission.

/// Emit a simple event in one line.
///
/// ```ignore
/// telemetry_event!(telemetry, telemetry::Level::Info, "system.startup", "started");
/// ```
#[macro_export]
macro_rules! telemetry_event {
    ($telemetry:expr, $level:expr, $kind:expr, $message:expr) => {
        $crate::event::emit_event($telemetry, $level, $kind, $message)
    };
}