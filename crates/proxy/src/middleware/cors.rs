//! CORS middleware stub.

use core::{Error as WrapperError, Result};

/// Adds `access-control-allow-origin` to responses.
#[derive(Clone, Debug)]
pub struct CorsMiddleware {
    pub allow_origin: String,
}

impl CorsMiddleware {
    pub fn new(allow_origin: impl Into<String>) -> Self {
        Self {
            allow_origin: allow_origin.into(),
        }
    }

    pub fn apply(
        &self,
        res: &mut hyper::Response<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<()> {
        let value = self
            .allow_origin
            .parse::<hyper::header::HeaderValue>()
            .map_err(|e| WrapperError::Proxy(format!("bad cors origin: {e}")))?;
        res.headers_mut()
            .insert(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        Ok(())
    }

    pub fn preflight() -> hyper::Response<http_body_util::Full<bytes::Bytes>> {
        crate::http::body::full_response(
            hyper::StatusCode::NO_CONTENT,
            bytes::Bytes::new(),
        )
    }
}
