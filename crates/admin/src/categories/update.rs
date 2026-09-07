//! Update a category (row in the global `categories` table).

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// Patch for [`update_category`]; `None` fields are left unchanged.
#[derive(Debug, Clone, Default)]
pub struct UpdateCategory {
    pub name: Option<String>,
    pub description: Option<String>,
    pub vm_template_id: Option<String>,
    pub max_tenants: Option<i64>,
}

/// Apply the patch via `db::global::categories::update` and emit
/// `admin.category.updated`.
pub async fn update_category(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    id: &str,
    patch: UpdateCategory,
) -> Result<()> {
    if id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category id must not be empty".into(),
        ));
    }
    if let Some(name) = &patch.name
        && name.trim().is_empty()
    {
        return Err(Error::InvalidInput(
            "category name must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let mut category = db::global::categories::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| Error::NotFound(format!("category {id}")))?;
    if let Some(name) = patch.name {
        category.name = name;
    }
    if let Some(description) = patch.description {
        category.description = Some(description);
    }
    if let Some(vm_template_id) = patch.vm_template_id {
        category.vm_template_id = vm_template_id;
    }
    if patch.max_tenants.is_some() {
        category.max_tenants = patch.max_tenants;
    }
    category.updated_at = core::now_rfc3339();
    db::global::categories::update(&conn, &category)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.category.updated")
        .category("admin")
        .source("admin::categories::update")
        .message(format!("category {id} updated"))
        .data("category_id", id.to_string())
        .tag("admin")
        .tag("category")
        .emit();
    Ok(())
}
