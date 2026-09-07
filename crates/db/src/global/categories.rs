//! CRUD for the `categories` table.

use core::Category;
use turso::Connection;

pub async fn insert(conn: &Connection, c: &Category) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO categories (id, name, description, vm_template_id, max_tenants, status, \
         created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        (
            c.id.as_str(), c.name.as_str(), c.description.as_deref().unwrap_or(""),
            c.vm_template_id.as_str(), c.max_tenants.unwrap_or(0), c.status.as_str(),
            c.created_at.as_str(), c.updated_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection) -> anyhow::Result<Vec<Category>> {
    let mut rows = conn.query("SELECT id, name, description, vm_template_id, max_tenants, status, \
        created_at, updated_at FROM categories ORDER BY name", ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        out.push(Category {
            id: text(0), name: text(1),
            description: Some(text(2)).filter(|s| !s.is_empty()),
            vm_template_id: text(3),
            max_tenants: row.get_value(4).ok().and_then(|v| v.as_integer().copied()),
            status: core::CategoryStatus::Active,
            created_at: text(6), updated_at: text(7),
        });
    }
    Ok(out)
}

pub async fn update(conn: &Connection, c: &Category) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE categories SET name = ?1, description = ?2, vm_template_id = ?3, max_tenants = ?4, \
         status = ?5, updated_at = ?6 WHERE id = ?7",
        (
            c.name.as_str(), c.description.as_deref().unwrap_or(""),
            c.vm_template_id.as_str(), c.max_tenants.unwrap_or(0), c.status.as_str(),
            c.updated_at.as_str(), c.id.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn delete(conn: &Connection, id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM categories WHERE id = ?1", [id]).await?;
    Ok(())
}
