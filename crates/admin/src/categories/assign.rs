//! Assign a tenant to a category (row in `category_assignments` plus the
//! tenant's own `category_id`).

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// One row of the `category_assignments` table.
#[derive(Debug, Clone)]
pub struct CategoryAssignment {
    pub tenant_id: String,
    pub category_id: String,
    pub assigned_by: Option<String>,
    pub notes: Option<String>,
}

/// Persist the assignment via `db::global::category_assignments::insert`,
/// move the tenant row to the new category, and emit `admin.category.updated`
/// plus `admin.tenant.category_changed`.
pub async fn assign_category(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    assignment: CategoryAssignment,
) -> Result<()> {
    if assignment.tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "tenant_id must not be empty".into(),
        ));
    }
    if assignment.category_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let row = db::global::CategoryAssignmentRow {
        id: format!("asg_{}", uuid::Uuid::new_v4()),
        tenant_id: assignment.tenant_id.clone(),
        category_id: assignment.category_id.clone(),
        assigned_by: assignment.assigned_by.clone(),
        assigned_at: core::now_rfc3339(),
        migrated: false,
        notes: assignment.notes.clone(),
    };
    db::global::category_assignments::insert(&conn, &row)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let mut tenant = db::global::tenants::get_by_id(&conn, &assignment.tenant_id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::NotFound(format!("tenant {}", assignment.tenant_id)))?;
    tenant.category_id = assignment.category_id.clone();
    tenant.category_changed_at = Some(core::now_rfc3339());
    db::global::tenants::update(&conn, &tenant)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.category.updated")
        .category("admin")
        .source("admin::categories::assign")
        .message(format!(
            "category {} assigned (tenant {})",
            assignment.category_id, assignment.tenant_id
        ))
        .data("category_id", assignment.category_id.clone())
        .tenant_id(Some(assignment.tenant_id.clone()))
        .tag("admin")
        .tag("category")
        .emit();
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.tenant.category_changed")
        .category("admin")
        .source("admin::categories::assign")
        .message(format!(
            "tenant {} assigned to category {}",
            assignment.tenant_id, assignment.category_id
        ))
        .tenant_id(Some(assignment.tenant_id.clone()))
        .data("category_id", assignment.category_id.clone())
        .tag("admin")
        .tag("category")
        .emit();
    Ok(())
}
