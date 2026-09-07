//! Request logging middleware stub.

use core::Result;
use telemetry::{Level, Telemetry};

/// Emits one telemetry event per request.
#[derive(Clone, Debug)]
pub struct LoggingMiddleware {
    telemetry: Telemetry,
}

impl LoggingMiddleware {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    pub async fn log_request(&self, req: &hyper::Request<hyper::body::Incoming>) -> Result<()> {
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.received")
            .category("proxy")
            .source("proxy::middleware::logging")
            .message(format!("{} {}", req.method(), req.uri().path()))
            .emit();
        Ok(())
    }
}
