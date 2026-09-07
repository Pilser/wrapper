//! Request transform: add headers and strip a path prefix.

use shared::{Error as WrapperError, Result};
use telemetry::{Level, Telemetry};

/// Mutates incoming requests before forwarding.
#[derive(Clone, Debug, Default)]
pub struct RequestTransform {
    pub add_headers: Vec<(String, String)>,
    pub strip_prefix: Option<String>,
    telemetry: Option<Telemetry>,
}

impl RequestTransform {
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

    pub fn apply(&self, req: &mut hyper::Request<hyper::body::Incoming>) -> Result<()> {
        for (name, value) in &self.add_headers {
            let header_value = value
                .parse::<hyper::header::HeaderValue>()
                .map_err(|e| WrapperError::Proxy(format!("bad header value: {e}")))?;
            if let Ok(header_name) = name.parse::<hyper::header::HeaderName>() {
                req.headers_mut().insert(header_name, header_value);
            }
        }
        if let Some(prefix) = &self.strip_prefix {
            let path = req.uri().path().to_string();
            if let Some(stripped) = path.strip_prefix(prefix) {
                let new_path = if stripped.is_empty() { "/" } else { stripped };
                let query = req
                    .uri()
                    .query()
                    .map(|q| format!("?{q}"))
                    .unwrap_or_default();
                let uri: hyper::Uri = format!("{new_path}{query}")
                    .parse()
                    .map_err(|e| WrapperError::Proxy(format!("bad stripped uri: {e}")))?;
                *req.uri_mut() = uri;
            }
        }
        if let Some(telemetry) = &self.telemetry {
            telemetry
                .builder()
                .level(Level::Info)
                .kind("proxy.request.forwarded")
                .category("proxy")
                .source("proxy::transform::request")
                .message("request transform applied")
                .emit();
        }
        Ok(())
    }
}
