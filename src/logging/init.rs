//! Logging initialization (stderr; no external logger wired yet).

/// Initialize logging. Currently a marker: events go to stderr via
/// [`super::format::format_event`].
pub fn init() {
    eprintln!("{}", super::format::format_event("info", "logging", "logging initialized"));
}
