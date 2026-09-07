//! Logical WebSocket stream stub (no real socket; telemetry only).

use core::Result;
use telemetry::{Level, Telemetry};

/// One logical client connection.
#[derive(Clone, Debug)]
pub struct WsStream {
    pub id: String,
    telemetry: Telemetry,
}

impl WsStream {
    pub fn new(id: impl Into<String>, telemetry: Telemetry) -> Self {
        Self {
            id: id.into(),
            telemetry,
        }
    }

    pub async fn send_text(&self, text: &str) -> Result<()> {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.websocket.message_sent")
            .category("proxy")
            .source("proxy::websocket::stream")
            .message(format!("ws send id={} len={}", self.id, text.len()))
            .emit();
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.websocket.disconnected")
            .category("proxy")
            .source("proxy::websocket::stream")
            .message(format!("ws close id={}", self.id))
            .emit();
        Ok(())
    }
}
