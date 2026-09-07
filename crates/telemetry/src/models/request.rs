//! Request payload attached to proxy request events.

use serde::{Serialize, Deserialize};

/// HTTP request tracking data (received and completed variants).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestPayload {
    pub method: String,
    pub path: String,
    /// Set on completion.
    pub status_code: Option<u16>,
    pub duration_ms: Option<f64>,
    pub bytes_sent: Option<u64>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub content_length: Option<u64>,
}