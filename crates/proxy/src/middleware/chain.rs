//! Ordered middleware chain stub.

use core::Result;
use telemetry::{Level, Telemetry};

/// Named checks run in order. Stub: logs names and returns Ok.
#[derive(Clone, Debug)]
pub struct MiddlewareChain {
    pub names: Vec<String>,
    telemetry: Telemetry,
}

impl MiddlewareChain {
    pub fn new(telemetry: Telemetry) -> Self {
        Self {
            names: Vec::new(),
            telemetry,
        }
    }

    pub fn push(&mut self, name: impl Into<String>) {
        self.names.push(name.into());
    }

    pub async fn run(&self, req: &hyper::Request<hyper::body::Incoming>) -> Result<()> {
        for name in &self.names {
            self.telemetry
                .builder()
                .level(Level::Info)
                .kind("proxy.request.received")
                .category("proxy")
                .source("proxy::middleware::chain")
                .message(format!("middleware {name} path={}", req.uri().path()))
                .emit();
        }
        Ok(())
    }
}
