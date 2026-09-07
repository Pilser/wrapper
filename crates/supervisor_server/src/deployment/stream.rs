use core::{Error, Result};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use telemetry::{Level, Telemetry};

/// In-flight deployment byte buffers, keyed by id (tenant or deployment id
/// supplied by the caller). Chunks accumulate here until the receiver drains
/// them with [`take_buffer`] for verification/assembly.
static BUFFERS: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();

fn buffers() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    BUFFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Append one streamed deployment chunk to the in-flight buffer for `id`.
pub async fn append_chunk(
    id: &str,
    chunk_index: u64,
    data: &[u8],
    telemetry: &Telemetry,
) -> Result<()> {
    if id.trim().is_empty() {
        return Err(Error::InvalidInput("deployment chunk missing `id`".to_string()));
    }
    let len = {
        let mut guard = buffers()
            .lock()
            .map_err(|e| Error::Internal(format!("chunk buffer lock: {e}")))?;
        let buf = guard.entry(id.to_string()).or_default();
        buf.extend_from_slice(data);
        buf.len()
    };
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.deployment.sent")
        .category("supervisor")
        .message(format!(
            "deployment chunk {chunk_index} streamed id {id} ({len} bytes buffered)"
        ))
        .emit();
    Ok(())
}

/// Bytes currently buffered for `id`.
pub fn buffered_len(id: &str) -> usize {
    buffers()
        .lock()
        .ok()
        .and_then(|guard| guard.get(id).map(|buf| buf.len()))
        .unwrap_or(0)
}

/// Drain and return the buffered bytes for `id`, if any.
pub fn take_buffer(id: &str) -> Option<Vec<u8>> {
    buffers().lock().ok()?.remove(id)
}

pub fn total_chunks(payload: &serde_json::Value) -> Option<u64> {
    payload.get("total_chunks").and_then(|v| v.as_u64())
}
