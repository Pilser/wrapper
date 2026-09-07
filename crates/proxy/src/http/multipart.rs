//! Multipart form-data stubs.

use shared::Result;
use telemetry::{Level, Telemetry};

/// One parsed multipart field (stub: caller fills it in).
#[derive(Clone, Debug)]
pub struct MultipartField {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub bytes: bytes::Bytes,
}

/// Extract the `boundary=` parameter from a multipart content type.
pub fn boundary_of(content_type: &str) -> Result<Option<String>> {
    for part in content_type.split(';') {
        let part = part.trim();
        if let Some(boundary) = part.strip_prefix("boundary=") {
            let boundary = boundary.trim().trim_matches('"').to_string();
            if boundary.is_empty() {
                return Ok(None);
            }
            return Ok(Some(boundary));
        }
    }
    Ok(None)
}

/// Log received fields (no real parsing yet).
pub async fn note_fields(telemetry: &Telemetry, fields: &[MultipartField]) -> Result<()> {
    telemetry
        .builder()
        .level(Level::Info)
        .kind("proxy.request.received")
        .category("proxy")
        .source("proxy::http::multipart")
        .message(format!("multipart fields count={}", fields.len()))
        .emit();
    Ok(())
}
