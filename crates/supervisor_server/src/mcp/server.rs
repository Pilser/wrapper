use telemetry::Telemetry;

use crate::protocol::message::Envelope;

/// Wrapper-side MCP bridge: validates, dispatches and answers tool calls.
#[derive(Clone)]
pub struct McpServer {
    pub telemetry: Telemetry,
    pub db: db::Db,
}

impl McpServer {
    pub fn new(telemetry: Telemetry, db: db::Db) -> Self {
        Self { telemetry, db }
    }

    pub fn tool_names(&self) -> Vec<String> {
        super::tools::tool_names()
    }

    pub async fn handle_envelope(&self, envelope: &Envelope) -> Envelope {
        match super::handler::dispatch(envelope, &self.db, &self.telemetry).await {
            Ok(payload) => super::handler::success_response(envelope, payload),
            Err(e) => super::handler::error_response(envelope, &e),
        }
    }
}
