//! WebSocket ping/pong keepalive stub.

use shared::Result;
use telemetry::{Level, Telemetry};

/// Tracks the last pong; stub only.
#[derive(Clone, Debug)]
pub struct PingTracker {
    pub interval_secs: u64,
    pub last_pong: Option<String>,
    telemetry: Telemetry,
}

impl PingTracker {
    pub fn new(interval_secs: u64, telemetry: Telemetry) -> Self {
        Self {
            interval_secs,
            last_pong: None,
            telemetry,
        }
    }

    pub async fn ping(&self, id: &str) -> Result<()> {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.websocket.message_sent")
            .category("proxy")
            .source("proxy::websocket::ping")
            .message(format!("ws ping id={id} every={}s", self.interval_secs))
            .emit();
        Ok(())
    }

    pub fn on_pong(&mut self) {
        self.last_pong = Some(shared::now_rfc3339());
    }
}
