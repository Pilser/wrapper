//! Header helpers: copy and hop-by-hop sanitization.

/// Copy one header value from `from` to `to` (no-op if missing/invalid).
pub fn copy_header(
    from: &hyper::header::HeaderMap,
    to: &mut hyper::header::HeaderMap,
    name: &str,
) {
    if let Ok(header_name) = name.parse::<hyper::header::HeaderName>() {
        if let Some(value) = from.get(&header_name) {
            to.insert(header_name, value.clone());
        }
    }
}

/// Remove hop-by-hop headers that must not be forwarded.
pub fn sanitize(headers: &mut hyper::header::HeaderMap) {
    for name in ["connection", "keep-alive", "transfer-encoding", "upgrade"] {
        headers.remove(name);
    }
}
