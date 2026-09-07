//! Streaming response stub (concatenate chunks + telemetry).

use shared::Result;
use telemetry::{Level, Telemetry};

/// One chunk of a streamed response.
#[derive(Clone, Debug)]
pub struct StreamedChunk {
    pub bytes: bytes::Bytes,
}

/// Collect chunks into a single buffer (stub for chunked forwarding).
pub async fn collect_streamed(
    chunks: Vec<bytes::Bytes>,
    telemetry: &Telemetry,
) -> Result<bytes::Bytes> {
    let total: usize = chunks.iter().map(|c| c.len()).sum();
    let mut out = Vec::with_capacity(total);
    for chunk in chunks {
        out.extend_from_slice(&chunk);
    }
    telemetry
        .builder()
        .level(Level::Info)
        .kind("proxy.response.sent")
        .category("proxy")
        .source("proxy::http::streaming")
        .message(format!("streamed response bytes={total}"))
        .emit();
    Ok(bytes::Bytes::from(out))
}
