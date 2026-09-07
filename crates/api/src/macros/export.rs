//! Symbols and helpers exported to dynamically loaded plugins.

/// Emit an `INFO` telemetry event from a plugin call site.
///
/// Usage: `api_emit!(ctx.telemetry, "plugin.custom.event", "did something");`
#[macro_export]
macro_rules! api_emit {
    ($telemetry:expr, $kind:expr, $message:expr) => {
        $telemetry
            .builder()
            .level(::telemetry::Level::Info)
            .kind($kind)
            .category("plugin")
            .message($message)
            .emit()
    };
}

/// Names of the API symbols plugin `.so` files may import.
pub fn exported_symbols() -> Vec<&'static str> {
    vec![
        "api_context",
        "api_emit",
        "db",
        "vm",
        "network",
        "proxy",
        "auth",
        "supervisor",
        "admin",
        "telemetry",
        "plugin",
        "events",
    ]
}
