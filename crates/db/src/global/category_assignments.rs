//! CRUD for the `category_assignments` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

/// One row of the global `category_assignments` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryAssignmentRow {
    pub id: String,
    pub tenant_id: String,
    pub category_id: String,
    pub assigned_by: Option<String>,
    pub assigned_at: String,
    pub migrated: bool,
    pub notes: Option<String>,
}

pub async fn insert(conn: &Connection, a: &CategoryAssignmentRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO category_assignments (id, tenant_id, category_id, assigned_by, assigned_at, \
         migrated, notes) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (
            a.id.as_str(), a.tenant_id.as_str(), a.category_id.as_str(),
            a.assigned_by.as_deref().unwrap_or(""), a.assigned_at.as_str(),
            if a.migrated { 1i64 } else { 0i64 },
            a.notes.as_deref().unwrap_or(""),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection) -> anyhow::Result<Vec<CategoryAssignmentRow>> {
    let mut rows = conn.query("SELECT id, tenant_id, category_id, assigned_by, assigned_at, \
        migrated, notes FROM category_assignments ORDER BY assigned_at", ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        out.push(CategoryAssignmentRow {
            id: text(0), tenant_id: text(1), category_id: text(2),
            assigned_by: Some(text(3)).filter(|s| !s.is_empty()),
            assigned_at: text(4),
            migrated: row.get_value(5).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0) != 0,
            notes: Some(text(6)).filter(|s| !s.is_empty()),
        });
    }
    Ok(out)
}

pub async fn delete_by_tenant(conn: &Connection, tenant_id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM category_assignments WHERE tenant_id = ?1", [tenant_id]).await?;
    Ok(())
}
