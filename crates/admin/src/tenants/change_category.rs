//! Change a tenant's category (updates the `tenants` row and records the
//! move in `category_assignments`).
//!
//! A real migration would additionally: look up the new category's VM
//! template, shut down the current VM if the template differs (the data disk
//! stays on the host), boot the new template with the same data disk, and
//! deploy the new category's package.

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// Input for [`change_category`].
#[derive(Debug, Clone)]
pub struct ChangeCategory {
    pub tenant_id: String,
    pub new_category_id: String,
    /// Migrate the VM to the new category's template when it differs.
    pub migrate_vm: bool,
}

/// Update the tenant row, record the assignment, and emit
/// `admin.tenant.category_changed`.
pub async fn change_category(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    input: ChangeCategory,
) -> Result<()> {
    if input.tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "tenant_id must not be empty".into(),
        ));
    }
    if input.new_category_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "new_category_id must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let mut tenant = db::global::tenants::get_by_id(&conn, &input.tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound(format!("tenant {}", input.tenant_id)))?;
    tenant.category_id = input.new_category_id.clone();
    tenant.category_changed_at = Some(core::now_rfc3339());
    db::global::tenants::update(&conn, &tenant)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let row = db::global::CategoryAssignmentRow {
        id: format!("asg_{}", uuid::Uuid::new_v4()),
        tenant_id: input.tenant_id.clone(),
        category_id: input.new_category_id.clone(),
        assigned_by: None,
        assigned_at: core::now_rfc3339(),
        migrated: input.migrate_vm,
        notes: None,
    };
    db::global::category_assignments::insert(&conn, &row)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.tenant.category_changed")
        .category("admin")
        .source("admin::tenants::change_category")
        .message(format!(
            "tenant {} moved to category {}",
            input.tenant_id, input.new_category_id
        ))
        .tenant_id(Some(input.tenant_id.clone()))
        .data("category_id", input.new_category_id.clone())
        .data("migrate_vm", input.migrate_vm.to_string())
        .tag("admin")
        .tag("tenant")
        .emit();
    Ok(())
}
