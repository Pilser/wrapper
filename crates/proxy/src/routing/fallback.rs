//! Fallback handler for unroutable requests (503 stub).

use shared::Result;
use telemetry::{Level, Telemetry};

/// Serves a 503 for requests no route handles.
#[derive(Clone, Debug)]
pub struct FallbackHandler {
    telemetry: Telemetry,
}

impl FallbackHandler {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    pub async fn handle(
        &self,
        req: &hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<http_body_util::Full<bytes::Bytes>>> {
        self.telemetry
            .builder()
            .level(Level::Error)
            .kind("proxy.request.failed")
            .category("proxy")
            .source("proxy::routing::fallback")
            .message(format!("no route for path={}", req.uri().path()))
            .emit();
        Ok(crate::http::body::full_response(
            hyper::StatusCode::SERVICE_UNAVAILABLE,
            bytes::Bytes::from_static(b"no upstream available"),
        ))
    }
}
