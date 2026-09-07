//! List VM templates via `db::global::vm_templates::list`.

use core::{Error, Result, VmTemplate};

/// Return every row of the global `vm_templates` table.
pub async fn list_templates(db: &db::GlobalDb) -> Result<Vec<VmTemplate>> {
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::vm_templates::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))
}
