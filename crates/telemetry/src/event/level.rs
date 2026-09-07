//! Severity level of an event.

use serde::{Deserialize, Serialize};

/// Log severity. Serialized as lowercase, e.g. `"info"`, `"error"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Critical,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Trace => "trace",
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
            Level::Critical => "critical",
        }
    }

    /// Parse from a lowercase string; defaults to `Info` on unknown input.
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Level::Trace,
            "debug" => Level::Debug,
            "warn" | "warning" => Level::Warn,
            "error" => Level::Error,
            "critical" | "fatal" => Level::Critical,
            _ => Level::Info,
        }
    }

    pub fn is_at_least(self, min: Level) -> bool {
        self >= min
    }
}