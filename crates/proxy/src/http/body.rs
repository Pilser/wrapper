//! HTTP body helpers.

use core::{Error as WrapperError, Result};

/// Collect an incoming request body into bytes.
pub async fn read_body(
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<bytes::Bytes> {
    use http_body_util::BodyExt;
    let bytes = req
        .into_body()
        .collect()
        .await
        .map_err(|e| WrapperError::Proxy(e.to_string()))?
        .to_bytes();
    Ok(bytes)
}

/// Build a full-buffered response.
pub fn full_response(
    status: hyper::StatusCode,
    bytes: bytes::Bytes,
) -> hyper::Response<http_body_util::Full<bytes::Bytes>> {
    hyper::Response::builder()
        .status(status)
        .body(http_body_util::Full::new(bytes))
        .unwrap_or_else(|_| {
            hyper::Response::new(http_body_util::Full::new(bytes::Bytes::from_static(
                b"proxy error",
            )))
        })
}

/// Convenience text response.
pub fn text_response(
    status: hyper::StatusCode,
    text: &str,
) -> hyper::Response<http_body_util::Full<bytes::Bytes>> {
    full_response(status, bytes::Bytes::copy_from_slice(text.as_bytes()))
}
