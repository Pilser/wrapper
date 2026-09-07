//! Plain-TCP listener for the proxy.

use core::Result;
use telemetry::{Level, Telemetry};

/// Binds a [`tokio::net::TcpListener`] for the proxy server.
#[derive(Clone, Debug)]
pub struct TcpAcceptor {
    pub bind_addr: String,
    telemetry: Telemetry,
}

impl TcpAcceptor {
    pub fn new(bind_addr: impl Into<String>, telemetry: Telemetry) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            telemetry,
        }
    }

    pub async fn bind(&self) -> Result<tokio::net::TcpListener> {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.received")
            .category("proxy")
            .source("proxy::server::tcp")
            .message(format!("binding tcp {}", self.bind_addr))
            .emit();
        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        Ok(listener)
    }
}
