//! A connected dashboard client streaming live events over WebSocket.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::SinkExt;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use super::filter::BroadcastFilter;
use super::server::Subscription;
use super::throttle::RateLimiter;
use crate::models::Event;

/// Opaque handle used to track a connected client.
#[derive(Debug, Clone)]
pub struct WsHandle(pub u64);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A WebSocket connection from a dashboard.
pub struct WsClient {
    pub id: u64,
    pub sub: Subscription,
    pub limiter: RateLimiter,
    ws: WebSocketStream<TcpStream>,
}

impl WsClient {
    pub async fn new(
        sub: Subscription,
        ws: WebSocketStream<TcpStream>,
        events_per_second: u32,
    ) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            sub,
            limiter: RateLimiter::new(events_per_second),
            ws,
        }
    }

    pub fn set_filter(&mut self, filter: BroadcastFilter) {
        self.sub.filter = filter;
    }

    pub fn handle(&self) -> WsHandle {
        WsHandle(self.id)
    }

    /// Pump events from the subscription into the socket until the client
    /// disconnects.
    pub async fn run(mut self) -> anyhow::Result<()> {
        loop {
            let next = self.sub.recv();
            use futures_util::StreamExt;
            tokio::select! {
                maybe = next => {
                    match maybe {
                        Some(ev) => {
                            if self.limiter.allow() {
                                if let Err(_e) = self.send(&ev).await {
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
                incoming = self.ws.next() => {
                    match incoming {
                        Some(Ok(Message::Text(_text))) => { /* ignore control traffic */ }
                        Some(Ok(_)) | None => break,
                        Some(Err(_)) => break,
                    }
                }
            }
        }
        Ok(())
    }

    async fn send(&mut self, ev: &Event) -> anyhow::Result<()> {
        let text = serde_json::to_string(ev)?;
        self.ws.send(Message::text(text)).await.map_err(|e| anyhow::anyhow!(e))
    }
}

/// Convenience constructor used by the server.
pub async fn accept(
    stream: TcpStream,
    sub: Subscription,
    events_per_second: u32,
) -> anyhow::Result<WsHandle> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let client = WsClient::new(sub, ws, events_per_second).await;
    let handle = client.handle();
    let _ = Arc::new(client); // ownership transferred below in real server
    Ok(handle)
}