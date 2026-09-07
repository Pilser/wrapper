//! Pooled upstream connection backed by a real TCP socket.
//!
//! [`PooledConn::connect`] opens a live [`tokio::net::TcpStream`] to the
//! target, so a pooled entry proves the upstream was reachable when created.
//! The socket can be taken out for a single proxied exchange
//! ([`PooledConn::take_stream`]) and returned afterwards
//! ([`PooledConn::put_stream`]); [`PooledConn::probe`] runs a fresh TCP
//! connect probe used by health checks.

use shared::{Error as WrapperError, Result};
use std::sync::Arc;
use std::time::Duration;
use telemetry::{Level, Telemetry};
use tokio::sync::Mutex;

/// One pooled upstream connection holding a live TCP socket.
#[derive(Clone, Debug)]
pub struct PooledConn {
    pub id: String,
    pub target_addr: String,
    pub created_at: String,
    stream: Arc<Mutex<Option<tokio::net::TcpStream>>>,
}

/// How long a health probe waits for a TCP connect.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

impl PooledConn {
    pub async fn connect(target_addr: &str, telemetry: &Telemetry) -> Result<Self> {
        telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.forwarded")
            .category("proxy")
            .source("proxy::pool::connection")
            .message(format!("connecting to upstream {target_addr}"))
            .emit();
        let stream = tokio::net::TcpStream::connect(target_addr)
            .await
            .map_err(|e| {
                WrapperError::Proxy(format!("connect {target_addr} failed: {e}"))
            })?;
        telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.forwarded")
            .category("proxy")
            .source("proxy::pool::connection")
            .message(format!("connected to upstream {target_addr}"))
            .emit();
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            target_addr: target_addr.to_string(),
            created_at: shared::now_rfc3339(),
            stream: Arc::new(Mutex::new(Some(stream))),
        })
    }

    /// Take the live socket out of the entry (leaves the slot empty).
    pub async fn take_stream(&self) -> Option<tokio::net::TcpStream> {
        self.stream.lock().await.take()
    }

    /// Return a socket to the entry.
    pub async fn put_stream(&self, stream: tokio::net::TcpStream) {
        *self.stream.lock().await = Some(stream);
    }

    /// Whether the entry currently holds a live socket.
    pub async fn has_stream(&self) -> bool {
        self.stream.lock().await.is_some()
    }

    /// Fresh TCP connect probe: can the target be dialed right now?
    pub async fn probe(&self) -> bool {
        tokio::time::timeout(PROBE_TIMEOUT, tokio::net::TcpStream::connect(&self.target_addr))
            .await
            .is_ok_and(|r| r.is_ok())
    }
}
