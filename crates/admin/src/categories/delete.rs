//! Delete a category (row in the global `categories` table).

use core::{Error, Result};
use telemetry::{Level, Telemetry};

/// Delete the row via `db::global::categories::delete` and emit
/// `admin.category.deleted`.
pub async fn delete_category(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    id: &str,
) -> Result<()> {
    if id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category id must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::categories::delete(&conn, id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Warn)
        .kind("admin.category.deleted")
        .category("admin")
        .source("admin::categories::delete")
        .message(format!("category {id} deleted"))
        .data("category_id", id.to_string())
        .tag("admin")
        .tag("category")
        .emit();
    Ok(())
}
