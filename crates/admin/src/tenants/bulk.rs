//! Bulk tenant operations (real writes, one aggregate telemetry event).

use shared::{Error, Result};
use telemetry::{Level, Telemetry};

/// Move many tenants to one category via `db::global::tenants::update`
/// (recording each move in `category_assignments`); returns the number of
/// tenants actually updated (unknown ids are skipped).
pub async fn bulk_update_category(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    tenant_ids: &[String],
    category_id: &str,
) -> Result<usize> {
    if category_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id must not be empty".into(),
        ));
    }
    if tenant_ids.iter().any(|id| id.trim().is_empty()) {
        return Err(Error::InvalidInput(
            "tenant ids must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let now = shared::now_rfc3339();
    let mut changed = 0usize;
    for tenant_id in tenant_ids {
        let Some(mut tenant) = db::global::tenants::get_by_id(&conn, tenant_id)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
        else {
            continue;
        };
        tenant.category_id = category_id.to_string();
        tenant.category_changed_at = Some(now.clone());
        db::global::tenants::update(&conn, &tenant)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        let row = db::global::CategoryAssignmentRow {
            id: format!("asg_{}", uuid::Uuid::new_v4()),
            tenant_id: tenant_id.clone(),
            category_id: category_id.to_string(),
            assigned_by: None,
            assigned_at: now.clone(),
            migrated: false,
            notes: None,
        };
        db::global::category_assignments::insert(&conn, &row)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
        changed += 1;
    }
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.tenant.category_changed")
        .category("admin")
        .source("admin::tenants::bulk")
        .message(format!(
            "bulk category change: {changed} tenant(s) to {category_id}"
        ))
        .data("category_id", category_id.to_string())
        .data("tenant_count", changed.to_string())
        .tag("admin")
        .tag("tenant")
        .emit();
    Ok(changed)
}
