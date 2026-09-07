//! Single-line log format: `<timestamp> [<level>] <target>: <message>`.

/// Format one log line with an RFC3339 timestamp.
pub fn format_event(level: &str, target: &str, message: &str) -> String {
    format!(
        "{} [{level}] {target}: {message}",
        core::now_rfc3339()
    )
}
