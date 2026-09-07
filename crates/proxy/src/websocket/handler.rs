//! WebSocket message handler: relays messages to the tenant's VM agent.
//!
//! Each message is forwarded to the upstream VM socket resolved via
//! [`VmLookup`](crate::routing::VmLookup) (`ws://<vm addr><upstream_path>`)
//! and the upstream reply is returned to the client, with telemetry emitted
//! for connect/message/failure points.

use core::{Error as WrapperError, Result};
use futures_util::{SinkExt, StreamExt};
use telemetry::{Level, Telemetry};
use tokio_tungstenite::tungstenite::Message;

/// Relays decoded WebSocket messages to the tenant's VM agent.
#[derive(Clone)]
pub struct WsHandler {
    telemetry: Telemetry,
    lookup: crate::routing::VmLookup,
    upstream_path: String,
}

impl std::fmt::Debug for WsHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WsHandler")
            .field("lookup", &self.lookup)
            .field("upstream_path", &self.upstream_path)
            .finish()
    }
}

impl WsHandler {
    pub fn new(telemetry: Telemetry) -> Self {
        Self {
            lookup: crate::routing::VmLookup::new(telemetry.clone()),
            upstream_path: "/ws".to_string(),
            telemetry,
        }
    }

    /// Wire VM address resolution (attach a DB-backed [`VmLookup`](crate::routing::VmLookup)).
    pub fn with_lookup(mut self, lookup: crate::routing::VmLookup) -> Self {
        self.lookup = lookup;
        self
    }

    /// Override the path dialed on the upstream VM agent.
    pub fn with_upstream_path(mut self, path: impl Into<String>) -> Self {
        self.upstream_path = path.into();
        self
    }

    pub async fn on_connect(&self, id: &str) -> Result<()> {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.websocket.connected")
            .category("proxy")
            .source("proxy::websocket::handler")
            .message(format!("websocket connected id={id}"))
            .emit();
        Ok(())
    }

    pub async fn on_disconnect(&self, id: &str) -> Result<()> {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.websocket.disconnected")
            .category("proxy")
            .source("proxy::websocket::handler")
            .message(format!("websocket disconnected id={id}"))
            .emit();
        Ok(())
    }

    /// Relay a client message to the `default` tenant's VM agent.
    pub async fn handle_message(&self, msg: Message) -> Result<Option<Message>> {
        self.handle_message_for("default", msg).await
    }

    /// Relay a client message to `tenant_id`'s VM agent and return its reply.
    pub async fn handle_message_for(
        &self,
        tenant_id: &str,
        msg: Message,
    ) -> Result<Option<Message>> {
        let summary = if msg.is_text() {
            "text"
        } else if msg.is_binary() {
            "binary"
        } else if msg.is_ping() {
            "ping"
        } else if msg.is_pong() {
            "pong"
        } else {
            "control"
        };
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.websocket.message_received")
            .category("proxy")
            .source("proxy::websocket::handler")
            .message(format!(
                "websocket message received kind={summary} tenant={tenant_id}"
            ))
            .emit();
        if msg.is_close() {
            return Ok(None);
        }
        match self.forward(tenant_id, msg).await {
            Ok(reply) => {
                let reply_kind = match &reply {
                    Some(m) if m.is_text() => "text",
                    Some(m) if m.is_binary() => "binary",
                    Some(_) => "control",
                    None => "close",
                };
                self.telemetry
                    .builder()
                    .level(Level::Info)
                    .kind("proxy.websocket.message_sent")
                    .category("proxy")
                    .source("proxy::websocket::handler")
                    .message(format!(
                        "websocket upstream reply kind={reply_kind} tenant={tenant_id}"
                    ))
                    .emit();
                Ok(reply)
            }
            Err(err) => {
                self.telemetry
                    .builder()
                    .level(Level::Error)
                    .kind("proxy.request.failed")
                    .category("proxy")
                    .source("proxy::websocket::handler")
                    .message(format!("websocket forward tenant={tenant_id} failed: {err}"))
                    .emit();
                Err(err)
            }
        }
    }

    /// Dial the tenant's VM agent, send one message, return its next reply.
    async fn forward(&self, tenant_id: &str, msg: Message) -> Result<Option<Message>> {
        let addr = self.lookup.lookup(tenant_id).await?;
        let url = format!("ws://{addr}{}", self.upstream_path);
        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| {
                WrapperError::Proxy(format!("ws upstream dial {addr} failed: {e}"))
            })?;
        ws.send(msg)
            .await
            .map_err(|e| WrapperError::Proxy(format!("ws upstream send {addr} failed: {e}")))?;
        let reply = ws.next().await;
        let _ = ws.close(None).await;
        match reply {
            Some(Ok(msg)) if msg.is_close() => Ok(None),
            Some(Ok(msg)) => Ok(Some(msg)),
            Some(Err(e)) => Err(WrapperError::Proxy(format!(
                "ws upstream recv {addr} failed: {e}"
            ))),
            None => Ok(None),
        }
    }
}
