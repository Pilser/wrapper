//! Minimal HTTP/1.1 client over firecracker's Unix socket.

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Talks JSON over HTTP/1.1 to a Firecracker instance's control socket.
pub struct FirecrackerClient {
    socket: String,
}

impl FirecrackerClient {
    pub fn new(socket: impl Into<String>) -> Self {
        Self { socket: socket.into() }
    }

    pub fn socket(&self) -> &str {
        &self.socket
    }

    /// Send `method` to `path` with optional JSON body; returns the body.
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> anyhow::Result<Value> {
        let raw = encode_http_request(method, path, body);
        let stream = tokio::net::UnixStream::connect(std::path::Path::new(&self.socket)).await?;
        let (mut rx, mut tx) = stream.into_split();
        tx.write_all(&raw).await?;
        tx.shutdown().await?;
        let mut buf = Vec::new();
        rx.read_to_end(&mut buf).await?;
        decode_http_body(&buf)
    }

    pub async fn put(&self, path: &str, body: &Value) -> anyhow::Result<Value> {
        self.request("PUT", path, Some(body)).await
    }

    pub async fn patch(&self, path: &str, body: &Value) -> anyhow::Result<Value> {
        self.request("PATCH", path, Some(body)).await
    }

    pub async fn get(&self, path: &str) -> anyhow::Result<Value> {
        self.request("GET", path, None).await
    }

    /// Firecracker `/actions` helper (InstanceStart, SendCtrlAltDel, FlushMetrics).
    pub async fn put_action(&self, action_type: &str) -> anyhow::Result<Value> {
        let body = serde_json::json!({ "action_type": action_type });
        self.put("/actions", &body).await
    }

    /// Firecracker `PUT /snapshot/create` helper.
    pub async fn create_snapshot(
        &self,
        snapshot_path: &str,
        mem_file_path: &str,
    ) -> anyhow::Result<Value> {
        let body = serde_json::json!({
            "snapshot_path": snapshot_path,
            "mem_file_path": mem_file_path,
            "snapshot_type": "Full",
        });
        self.put("/snapshot/create", &body).await
    }

    /// Firecracker `PUT /snapshot/load` helper.
    pub async fn load_snapshot(
        &self,
        snapshot_path: &str,
        mem_file_path: &str,
    ) -> anyhow::Result<Value> {
        let body = serde_json::json!({
            "snapshot_path": snapshot_path,
            "mem_file_path": mem_file_path,
        });
        self.put("/snapshot/load", &body).await
    }
}

/// Build a minimal HTTP/1.1 request frame.
pub fn encode_http_request(method: &str, path: &str, body: Option<&Value>) -> Vec<u8> {
    use std::io::Write;
    let mut req = Vec::new();
    let _ = write!(req, "{method} {path} HTTP/1.1\r\nHost: localhost\r\n");
    if let Some(b) = body {
        let json = b.to_string();
        let _ = write!(req, "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n", json.len());
        req.extend_from_slice(json.as_bytes());
    } else {
        let _ = write!(req, "\r\n");
    }
    req
}

/// Extract the JSON body from a firecracker HTTP response.
pub fn decode_http_body(buf: &[u8]) -> anyhow::Result<Value> {
    let text = String::from_utf8_lossy(buf);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
    let body = body.trim();
    if body.is_empty() {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_str(body)?)
}
