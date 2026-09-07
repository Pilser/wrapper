//! Create a category (row in the global `categories` table).

use core::{now_rfc3339, Category, CategoryStatus, Error, Result};
use telemetry::{Level, Telemetry};

/// Input for [`create_category`].
#[derive(Debug, Clone)]
pub struct CreateCategory {
    pub name: String,
    pub description: Option<String>,
    pub vm_template_id: String,
    pub max_tenants: Option<i64>,
}

/// Insert the category via `db::global::categories::insert`.
pub async fn create_category(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    input: CreateCategory,
) -> Result<Category> {
    if input.name.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category name must not be empty".into(),
        ));
    }
    if input.vm_template_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "vm_template_id must not be empty".into(),
        ));
    }
    let now = now_rfc3339();
    let category = Category {
        id: format!("cat_{}", uuid::Uuid::new_v4()),
        name: input.name,
        description: input.description,
        vm_template_id: input.vm_template_id,
        max_tenants: input.max_tenants,
        status: CategoryStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::categories::insert(&conn, &category)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.category.created")
        .category("admin")
        .source("admin::categories::create")
        .message(format!("category {} created", category.id))
        .data("category_id", category.id.clone())
        .tag("admin")
        .tag("category")
        .emit();
    Ok(category)
}
