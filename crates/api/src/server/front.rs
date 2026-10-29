//! Static file serving for the bundled web UI (config `[front]`).
//!
//! Files are read from the dist folder once at startup into memory (the
//! built UI is ~350 KB). The front uses hash routing, so the only routes it
//! needs are `/` (index.html) plus exact asset paths — anything else falls
//! through to the tenant zw proxy unchanged.

use std::collections::HashMap;
use std::path::Path;

use bytes::Bytes;
use http_body_util::Full;
use hyper::{header, StatusCode};

use super::{json, ApiResponse};

#[derive(Clone)]
pub struct Front {
    index: (String, Bytes),
    files: HashMap<String, (String, Bytes)>,
}

/// Content type by extension (covers everything vite emits plus common
/// extras like favicons and fonts).
fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "map" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

impl Front {
    /// Load every file under `dir` (recursive). Returns None when the
    /// folder is missing or has no `index.html` (front simply not bundled).
    pub fn load(dir: &str) -> Option<Front> {
        let root = Path::new(dir);
        let index_bytes = std::fs::read(root.join("index.html")).ok()?;
        let mut files = HashMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let Ok(bytes) = std::fs::read(&p) else {
                    continue;
                };
                let Ok(rel) = p.strip_prefix(root) else {
                    continue;
                };
                let rel = rel.to_string_lossy().replace('\\', "/");
                files.insert(
                    format!("/{rel}"),
                    (mime(&rel).to_string(), Bytes::from(bytes)),
                );
            }
        }
        Some(Front {
            index: ("text/html; charset=utf-8".to_string(), Bytes::from(index_bytes)),
            files,
        })
    }

    /// Number of asset files held (index excluded), for startup logging.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// `GET /` or an exact file match. Anything else → None so the caller
    /// falls through to the proxy (tenant zw apps stay reachable).
    pub fn respond(&self, path: &str) -> Option<ApiResponse> {
        if path == "/" || path == "/index.html" {
            return Some(self.reply(&self.index, false));
        }
        let decoded = decode_path(path);
        self.files
            .get(&decoded)
            .map(|f| self.reply(f, decoded.starts_with("/assets/")))
    }

    fn reply(&self, f: &(String, Bytes), immutable: bool) -> ApiResponse {
        let cache = if immutable {
            // vite emits content-hashed asset names: safe to cache forever.
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        hyper::Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, f.0.clone())
            .header(header::CACHE_CONTROL, cache)
            .header(header::CONNECTION, "close")
            .body(Full::new(f.1.clone()))
            .unwrap_or_else(|_| json::internal())
    }
}

/// Minimal percent-decoding for URL paths (`%2F` → `/`, `+` stays).
fn decode_path(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
