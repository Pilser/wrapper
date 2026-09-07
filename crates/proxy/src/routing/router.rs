//! Path/header router stub: maps a request to a tenant id.

use core::Result;
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
        if first.starts_with("tenant_") {
            return first.to_string();
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
