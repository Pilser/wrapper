//! List categories via `db::global::categories::list`.

use core::{Category, Error, Result};

/// Return every row of the global `categories` table ordered by name.
pub async fn list_categories(db: &db::GlobalDb) -> Result<Vec<Category>> {
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::categories::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))
}
