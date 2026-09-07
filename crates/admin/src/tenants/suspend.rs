//! Suspend a tenant (flips the `tenants` row status to `suspended`).

use shared::{Error, Result, TenantStatus};
use telemetry::{Level, Telemetry};

/// Set the tenant status to `suspended` via `db::global::tenants::update`
/// and emit the suspension.
pub async fn suspend_tenant(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    tenant_id: &str,
    reason: Option<String>,
) -> Result<()> {
    if tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "tenant_id must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let mut tenant = db::global::tenants::get_by_id(&conn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound(format!("tenant {tenant_id}")))?;
    tenant.status = TenantStatus::Suspended;
    db::global::tenants::update(&conn, &tenant)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Warn)
        .kind("admin.tenant.updated")
        .category("admin")
        .source("admin::tenants::suspend")
        .message(format!("tenant {tenant_id} suspended"))
        .tenant_id(Some(tenant_id.to_string()))
        .data("status", "suspended".to_string())
        .data("reason", reason.unwrap_or_default())
        .tag("admin")
        .tag("tenant")
        .emit();
    Ok(())
}
