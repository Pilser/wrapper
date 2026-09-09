//! Minimal HTTP/1.1 client over firecracker's Unix socket.

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time;

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

    /// Spawn the Firecracker process and wait until its API socket is ready.
    ///
    /// The process runs detached (its handle is leaked on purpose: the wrapper
    /// tracks VMs by socket path, and Firecracker keeps running as long as the
    /// host does). Returns a client bound to the socket once it exists.
    pub async fn spawn_firecracker(
        firecracker_bin: &str,
        api_sock: impl AsRef<std::path::Path>,
    ) -> anyhow::Result<Self> {
        let sock = api_sock.as_ref();
        // Remove a stale socket from a previous run.
        if sock.exists() {
            std::fs::remove_file(sock).map_err(|e| anyhow::anyhow!("remove stale sock: {e}"))?;
        }
        let sock_str = sock.to_string_lossy().to_string();
        // Firecracker 1.7 ABORTS at startup when its stdout is /dev/null
        // (silent SIGABRT, no socket ever appears). Redirect stdout+stderr
        // to a per-VM log file instead of null: this both avoids the abort
        // and keeps the guest serial console (kernel + /init + supervisor
        // output) for diagnosis. Sibling of the socket:
        // /tmp/fc_<tenant>.sock -> /tmp/fc_<tenant>.log.
        let log_path = sock.with_extension("log");
        let log_file = std::fs::File::create(&log_path)
            .map_err(|e| anyhow::anyhow!("create fc log {}: {e}", log_path.display()))?;
        let log_err = log_file
            .try_clone()
            .map_err(|e| anyhow::anyhow!("clone fc log handle: {e}"))?;
        // SAFETY: we leak the child handle deliberately. The Firecracker
        // process must outlive the wrapper's configuration calls; it is torn
        // down explicitly via /actions or by killing the PID later.
        let child = tokio::process::Command::new(firecracker_bin)
            .arg("--api-sock")
            .arg(&sock_str)
            .arg("--id")
            .arg(format!("fc-{}", std::process::id()))
            .stdin(std::process::Stdio::null())
            .stdout(log_file)
            .stderr(log_err)
            .spawn()
            .map_err(|e| anyhow::anyhow!("spawn firecracker: {e}"))?;
        // Keep the child running: forget the handle (we don't wait on it).
        std::mem::forget(child);

        // Wait for the API socket to appear (up to 5s).
        for _ in 0..50 {
            if sock.exists() {
                return Ok(Self::new(sock_str));
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Err(anyhow::anyhow!(
            "firecracker socket {sock_str} did not appear"
        ))
    }

    /// Send `method` to `path` with optional JSON body; returns the body.
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> anyhow::Result<Value> {
        let raw = encode_http_request(method, path, body);
        // The socket FILE appears microseconds before firecracker listen()s
        // on it; a single connect then fails ECONNREFUSED instantly and the
        // boot dies with no PUT ever reaching the server. Retry briefly.
        let mut last_err = String::new();
        for _ in 0..50 {
            match tokio::net::UnixStream::connect(std::path::Path::new(&self.socket)).await {
                Ok(stream) => {
                    let (mut rx, mut tx) = stream.into_split();
                    tx.write_all(&raw).await?;
                    tx.shutdown().await?;
                    let mut buf = Vec::new();
                    rx.read_to_end(&mut buf).await?;
                    return decode_http_body(&buf);
                }
                Err(e) => {
                    last_err = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
        Err(anyhow::anyhow!(
            "connect {} failed: {last_err}",
            self.socket
        ))
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
