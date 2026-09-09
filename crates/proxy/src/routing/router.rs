//! Path/header router stub: maps a request to a tenant id.

use shared::Result;
use telemetry::{Level, Telemetry};

/// Routes requests to tenants. Stub: header `x-tenant-id` or "default".
#[derive(Clone, Debug)]
pub struct Router {
    pub default_target: String,
    telemetry: Telemetry,
}

impl Router {
    pub fn new(default_target: impl Into<String>, telemetry: Telemetry) -> Self {
        Self {
            default_target: default_target.into(),
            telemetry,
        }
    }

    pub fn route(&self, req: &hyper::Request<hyper::body::Incoming>) -> String {
        if let Some(id) = req
            .headers()
            .get("x-tenant-id")
            .and_then(|v| v.to_str().ok())
        {
            return id.to_string();
        }
        let path = req.uri().path();
        let first = path.trim_start_matches('/').split('/').next().unwrap_or("");
        // Public links use /tenant_<id>/... but the canonical id is RAW
        // (DB rows, sockets, taps all key on it): strip the marker here so
        // header form ("abc") and path form ("/tenant_abc/...") converge.
        if let Some(raw) = first.strip_prefix("tenant_") {
            if !raw.is_empty() {
                return raw.to_string();
            }
        }
        // Absolute subresource URLs (zw emits /?asset=..., /favicon.*, ...):
        // the browser resolves them against the proxy origin ROOT, losing
        // the prefix. Recover the tenant from the Referer page
        // (http://host:18080/tenant_<id>/...), which browsers always send
        // same-origin. Without this every asset 500s (and the MIME error
        // below is just our JSON 500 served as .js/.css).
        if let Some(id) = tenant_from_url(req
            .headers()
            .get(hyper::header::REFERER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default())
        {
            return id;
        }
        // WebSocket handshakes send NO Referer (only Origin, which carries
        // no path), so root-path /ws can never resolve above. The proxy
        // therefore plants a zw_tenant cookie on every proxied page load
        // (see HttpHandler); browsers attach cookies to WS handshakes
        // automatically. Last resort before "default".
        if let Some(cookie) = req
            .headers()
            .get(hyper::header::COOKIE)
            .and_then(|v| v.to_str().ok())
        {
            for part in cookie.split(';') {
                let part = part.trim();
                if let Some(id) = part
                    .strip_prefix("zw_tenant=")
                    .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
                {
                    return id.to_string();
                }
            }
        }
        "default".to_string()
    }

    pub async fn route_with_telemetry(
        &self,
        req: &hyper::Request<hyper::body::Incoming>,
    ) -> Result<String> {
        let tenant_id = self.route(req);
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.received")
            .category("proxy")
            .source("proxy::routing::router")
            .message(format!("routed path={} tenant={tenant_id}", req.uri().path()))
            .emit();
        Ok(tenant_id)
    }
}

/// Extract a raw tenant id from any URL carrying a /tenant_<id>/ segment
/// (Referer pages, public links). None when absent.
fn tenant_from_url(url: &str) -> Option<String> {
    for seg in url.split('/') {
        if let Some(raw) = seg.strip_prefix("tenant_") {
            if !raw.is_empty() {
                return Some(raw.to_string());
            }
        }
    }
    None
}
