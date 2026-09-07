//! Delete a tenant: release real `sessions` / `networks` rows, delete the
//! `tenants` row, then emit `admin.tenant.deleted`.

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// Clear the tenant's sessions and network reservations, delete the tenant
/// row, then emit the event.
pub async fn delete_tenant(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    tenant_id: &str,
) -> Result<()> {
    if tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "tenant_id must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::sessions::delete_by_tenant(&conn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    db::global::networks::release_all(&conn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    db::global::category_assignments::delete_by_tenant(&conn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    db::global::tenants::delete(&conn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Warn)
        .kind("admin.tenant.deleted")
        .category("admin")
        .source("admin::tenants::delete")
        .message(format!("tenant {tenant_id} deleted"))
        .tenant_id(Some(tenant_id.to_string()))
        .tag("admin")
        .tag("tenant")
        .emit();
    Ok(())
}
