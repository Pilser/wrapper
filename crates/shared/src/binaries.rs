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

/// Newest mtime under the dir (top level + `zw-assets/`). Used to skip
/// re-hashing 140MB when nothing changed.
fn dir_mtime(dir: &std::path::Path) -> Option<std::time::SystemTime> {
    let mut newest: Option<std::time::SystemTime> = None;
    let mut scan = |d: &std::path::Path| {
        if let Ok(entries) = std::fs::read_dir(d) {
            for e in entries.flatten() {
                if let Ok(m) = e.metadata() {
                    if let Ok(t) = m.modified() {
                        newest = Some(newest.map_or(t, |cur| cur.max(t)));
                    }
                }
            }
        }
    };
    scan(dir);
    scan(&dir.join("zw-assets"));
    newest
}

static MANIFEST_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, (Option<std::time::SystemTime>, Vec<BinaryEntry>)>>,
> = std::sync::OnceLock::new();

/// Cached manifest: re-hashes only when the dir mtime moved. The binaries
/// folder holds ~140MB — hashing it on every supervisor poll burns host CPU.
pub fn manifest_cached(dir: &std::path::Path) -> Vec<BinaryEntry> {
    let mtime = dir_mtime(dir);
    if let Some(cache) = MANIFEST_CACHE.get() {
        if let Ok(guard) = cache.lock() {
            if let Some((t, entries)) = guard.get(dir) {
                if *t == mtime {
                    return entries.clone();
                }
            }
        }
    }
    let entries = manifest(dir);
    let cache = MANIFEST_CACHE
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(mut guard) = cache.lock() {
        guard.insert(dir.to_path_buf(), (mtime, entries.clone()));
    }
    entries
}

/// Walk the binaries dir (top level + one `zw-assets/` subdir) into a
/// manifest. Missing dir = empty manifest (nothing to demand yet).
pub fn manifest(dir: &std::path::Path) -> Vec<BinaryEntry> {
    let mut out = Vec::new();
    let mut walk = |rel: String, full: std::path::PathBuf| {
        let Ok(data) = std::fs::read(&full) else {
            return;
        };
        out.push(BinaryEntry {
            path: rel,
            sha256: sha256_bytes(&data),
            size: data.len() as u64,
        });
    };
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    walk(name.to_string(), p);
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
                    walk(format!("zw-assets/{name}"), p);
                }
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}
