//! Time helpers producing RFC3339 strings with nanosecond precision.

use chrono::{DateTime, SecondsFormat, Utc};

/// A timestamp represented as an RFC3339 string with nanosecond precision,
/// matching the format used throughout telemetry: `2026-09-06T14:30:00.123456789Z`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn now() -> Self {
        Self(Self::now_string())
    }

    pub fn now_string() -> String {
        Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Timestamp {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for Timestamp {
    fn from(v: String) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Current UTC time as RFC3339 with nanoseconds.
pub fn now_rfc3339() -> String {
    Timestamp::now_string()
}

/// Convert an `i64` millisecond value into a `u64`, clamping negatives to 0.
fn to_millis(ms: i64) -> u64 {
    u64::try_from(ms).unwrap_or(0)
}

/// Current unix time in milliseconds.
pub fn now_unix_millis() -> u64 {
    to_millis(Utc::now().timestamp_millis())
}

/// Parse an RFC3339 timestamp into unix milliseconds.
pub fn timestamp_to_unix_millis(ts: &str) -> Option<u64> {
    let parsed: Option<DateTime<Utc>> = DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|d| d.with_timezone(&Utc));
    parsed.map(|d| to_millis(d.timestamp_millis()))
}