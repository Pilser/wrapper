//! Tenant extraction and resolution stub.

use shared::Result;
use telemetry::{Level, Telemetry};

/// Where a tenant's traffic goes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TenantRoute {
    pub tenant_id: String,
    pub vm_addr: String,
}

/// Pull the tenant id from a request (header first, then path).
pub fn tenant_id_of(req: &hyper::Request<hyper::body::Incoming>) -> String {
    if let Some(id) = req
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
    {
        return id.to_string();
    }
    "default".to_string()
}

/// Resolve a tenant to a stub VM address.
pub async fn resolve(tenant_id: &str, telemetry: &Telemetry) -> Result<TenantRoute> {
    telemetry
        .builder()
        .level(Level::Info)
        .kind("proxy.request.forwarded")
        .category("proxy")
        .source("proxy::routing::tenant")
        .message(format!("resolved tenant={tenant_id}"))
        .emit();
    Ok(TenantRoute {
        tenant_id: tenant_id.to_string(),
        vm_addr: "127.0.0.1:8080".to_string(),
    })
}
