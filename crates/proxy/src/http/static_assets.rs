//! Local static asset serving for the wrapper.
//!
//! The app (zw) emits root-absolute asset URLs (`/_topcoat/assets/<file>`,
//! plus `/favicon.svg`). Those are meant for a front proxy that can serve
//! the shared asset bundle; without this, the wrapper treats them as tenant
//! requests and 500s with "wake VM for default: tenant default not found"
//! (or forwards to a guest which mangles `.mjs` MIME via `?asset=`).
//!
//! We serve them straight from the host binaries dir (`<binaries>/zw-assets`)
//! with correct MIME types by extension. Same-origin, no CORS, and it works
//! even before any tenant VM is awake. Only safe basenames under a small
//! allowlist of subdirectories (e.g. `fonts/`) are accepted — no path
//! traversal.

use bytes::Bytes;
use http_body_util::Full;
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE};
use std::path::{Path, PathBuf};
use tokio::fs;

/// URL prefix that maps onto the local asset bundle.
const ASSETS_PREFIX: &str = "/_topcoat/assets/";

/// Subdirectories allowed under the asset root (relative to `assets_root`).
const ALLOWED_SUBDIRS: &[&str] = &["fonts", "images", "icons", "locales"];

/// Extension -> MIME map (subset needed by the app).
fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// True when `path` targets the local asset zone.
pub fn is_asset_path(path: &str) -> bool {
    path.starts_with(ASSETS_PREFIX) || path == "/favicon.svg"
}

/// Map an asset URL path to a safe file under `assets_root`.
///
/// Rules: final file basename only; at most one subdirectory level, and it
/// must be on the allowlist; no `..`, no backslashes, no empty segments.
fn resolve_asset(assets_root: &Path, path: &str) -> Option<PathBuf> {
    let rel = if let Some(rest) = path.strip_prefix(ASSETS_PREFIX) {
        rest
    } else if path == "/favicon.svg" {
        "favicon.svg"
    } else {
        return None;
    };
    if rel.is_empty() || rel.contains("..") || rel.contains('\\') {
        return None;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.len() {
        1 => {
            let name = parts[0];
            if name.is_empty() {
                return None;
            }
            Some(assets_root.join(name))
        }
        2 => {
            let (dir, name) = (parts[0], parts[1]);
            if name.is_empty() || !ALLOWED_SUBDIRS.contains(&dir) {
                return None;
            }
            Some(assets_root.join(dir).join(name))
        }
        _ => None,
    }
}

/// Serve an asset file if it exists; `Ok(None)` means "not ours / missing".
pub async fn serve(
    assets_root: &Path,
    path: &str,
) -> shared::Result<Option<hyper::Response<Full<Bytes>>>> {
    let Some(file) = resolve_asset(assets_root, path) else {
        return Ok(None);
    };
    let data = match fs::read(&file).await {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    let len = data.len();
    let ctype = mime_for(&file);
    let resp = hyper::Response::builder()
        .status(hyper::StatusCode::OK)
        .header(CONTENT_TYPE, ctype)
        .header(CONTENT_LENGTH, len)
        .body(Full::new(Bytes::from(data)))
        .expect("static asset response");
    Ok(Some(resp))
}
