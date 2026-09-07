//! Create a tenant (row in the global `tenants` table).

use shared::{now_rfc3339, Error, Result, Tenant, TenantStatus};
use telemetry::{Level, Telemetry};

/// Input for [`create_tenant`].
#[derive(Debug, Clone)]
pub struct CreateTenant {
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub category_id: String,
}

/// Insert via `db::global::tenants::create` (which also emits `db.write.completed`).
pub async fn create_tenant(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    input: CreateTenant,
) -> Result<Tenant> {
    if input.username.trim().is_empty() {
        return Err(Error::InvalidInput(
            "username must not be empty".into(),
        ));
    }
    if input.password_hash.trim().is_empty() {
        return Err(Error::InvalidInput(
            "password_hash must not be empty".into(),
        ));
    }
    if input.category_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id must not be empty".into(),
        ));
    }
    let id = format!("ten_{}", uuid::Uuid::new_v4());
    let tenant = Tenant {
        id: id.clone(),
        username: input.username,
        password_hash: input.password_hash,
        display_name: input.display_name,
        category_id: input.category_id,
        vm_ip: None,
        vm_process_id: None,
        vm_template_id: None,
        tap_device: None,
        status: TenantStatus::Active,
        db_path: format!("data/tenants/{id}.db"),
        created_at: now_rfc3339(),
        last_login: None,
        category_changed_at: None,
    };
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::tenants::create(&conn, &tenant, &db.telemetry)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.tenant.created")
        .category("admin")
        .source("admin::tenants::create")
        .message(format!("tenant {} created", tenant.id))
        .tenant_id(Some(tenant.id.clone()))
        .tag("admin")
        .tag("tenant")
        .emit();
    Ok(tenant)
}
