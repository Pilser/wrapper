//! Guest binary distribution: manifest + path validation shared by the
//! HTTP admin API and the supervisor TCP channel.
//!
//! Layout under the configured binaries dir (flat + one assets subdir):
//! `zw`, `engram`, `zw-assets/<bundle files>`, `zw-assets/manifest.toml`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One distributed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// Accept only safe relative paths (no escapes, no absolute, sane charset).
pub fn validate_rel(path: &str) -> bool {
    if path.is_empty() || path.len() > 128 || path.starts_with('/') || path.contains("..") {
        return false;
    }
    path.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

pub fn sha256_bytes(data: &[u8]) -> String {
    hex_encode(Sha256::digest(data))
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Walk the binaries dir (top level + one `zw-assets/` subdir) into a
/// manifest. Missing dir = empty manifest (nothing to demand yet).
pub fn manifest(dir: &std::path::Path) -> Vec<BinaryEntry> {
    let mut out = Vec::new();
    let mut walk = |rel: &str, full: std::path::PathBuf| {
        let Ok(data) = std::fs::read(&full) else {
            return;
        };
        out.push(BinaryEntry {
            path: rel.to_string(),
            sha256: sha256_bytes(&data),
            size: data.len() as u64,
        });
    };
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    walk(name, p);
                }
            }
        }
    }
    let assets = dir.join("zw-assets");
    if let Ok(entries) = std::fs::read_dir(&assets) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    walk(&format!("zw-assets/{name}"), p);
                }
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}
