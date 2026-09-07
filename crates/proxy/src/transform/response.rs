//! Response transform: add headers before sending to the client.

use core::{Error as WrapperError, Result};
use telemetry::{Level, Telemetry};

/// Mutates outgoing responses.
#[derive(Clone, Debug, Default)]
pub struct ResponseTransform {
    pub add_headers: Vec<(String, String)>,
    telemetry: Option<Telemetry>,
}

impl ResponseTransform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_telemetry(mut self, telemetry: Telemetry) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    pub fn add_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.add_headers.push((name.into(), value.into()));
        self
    }

    pub fn apply(
        &self,
        res: &mut hyper::Response<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<()> {
        for (name, value) in &self.add_headers {
            let header_value = value
                .parse::<hyper::header::HeaderValue>()
                .map_err(|e| WrapperError::Proxy(format!("bad header value: {e}")))?;
            if let Ok(header_name) = name.parse::<hyper::header::HeaderName>() {
                res.headers_mut().insert(header_name, header_value);
            }
        }
        if let Some(telemetry) = &self.telemetry {
            telemetry
                .builder()
                .level(Level::Info)
                .kind("proxy.response.sent")
                .category("proxy")
                .source("proxy::transform::response")
                .message("response transform applied")
                .emit();
        }
        Ok(())
    }
}
