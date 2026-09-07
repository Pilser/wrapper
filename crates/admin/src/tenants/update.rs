//! Update a tenant (row in the global `tenants` table).

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// Patch for [`update_tenant`]; `None` fields are left unchanged.
#[derive(Debug, Clone, Default)]
pub struct UpdateTenant {
    pub display_name: Option<String>,
    pub password_hash: Option<String>,
}

/// Apply the patch via `db::global::tenants::update` and emit
/// `admin.tenant.updated`.
pub async fn update_tenant(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    tenant_id: &str,
    patch: UpdateTenant,
) -> Result<()> {
    if tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "tenant_id must not be empty".into(),
        ));
    }
    if let Some(hash) = &patch.password_hash
        && hash.trim().is_empty()
    {
        return Err(Error::InvalidInput(
            "password_hash must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let mut tenant = db::global::tenants::get_by_id(&conn, tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound(format!("tenant {tenant_id}")))?;
    if let Some(display_name) = patch.display_name {
        tenant.display_name = Some(display_name);
    }
    if let Some(password_hash) = patch.password_hash {
        tenant.password_hash = password_hash;
    }
    db::global::tenants::update(&conn, &tenant)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.tenant.updated")
        .category("admin")
        .source("admin::tenants::update")
        .message(format!("tenant {tenant_id} updated"))
        .tenant_id(Some(tenant_id.to_string()))
        .tag("admin")
        .tag("tenant")
        .emit();
    Ok(())
}
