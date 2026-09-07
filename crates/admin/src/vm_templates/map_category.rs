//! Map a category to its VM template (updates the category row).

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// Set `category_id`'s `vm_template_id` to `template_id` via
/// `db::global::categories::update` and emit `admin.category.updated`.
pub async fn map_category_to_template(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    category_id: &str,
    template_id: &str,
) -> Result<()> {
    if category_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id must not be empty".into(),
        ));
    }
    if template_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "template_id must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let mut category = db::global::categories::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .into_iter()
        .find(|c| c.id == category_id)
        .ok_or_else(|| Error::NotFound(format!("category {category_id}")))?;
    category.vm_template_id = template_id.to_string();
    category.updated_at = core::now_rfc3339();
    db::global::categories::update(&conn, &category)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.category.updated")
        .category("admin")
        .source("admin::vm_templates::map_category")
        .message(format!(
            "category {category_id} mapped to template {template_id}"
        ))
        .data("category_id", category_id.to_string())
        .data("template_id", template_id.to_string())
        .tag("admin")
        .tag("vm")
        .emit();
    Ok(())
}
