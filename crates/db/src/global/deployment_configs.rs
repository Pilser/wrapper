//! CRUD for the `deployment_configs` table.

use core::DeploymentConfig;
use turso::Connection;

pub async fn insert(conn: &Connection, d: &DeploymentConfig) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO deployment_configs (id, category_id, config_type, name, path, content, \
         checksum, priority, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        (
            d.id.as_str(), d.category_id.as_str(), d.config_type.as_str(),
            d.name.as_str(), d.path.as_str(),
            d.content.clone().unwrap_or_default(),
            d.checksum.as_deref().unwrap_or(""),
            d.priority as i64,
            d.created_at.as_str(), d.updated_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection) -> anyhow::Result<Vec<DeploymentConfig>> {
    let mut rows = conn.query("SELECT id, category_id, config_type, name, path, content, checksum, \
        priority, created_at, updated_at FROM deployment_configs ORDER BY priority, name", ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        out.push(DeploymentConfig {
            id: text(0), category_id: text(1), config_type: text(2),
            name: text(3), path: text(4),
            content: row.get_value(5).ok().and_then(|v| v.as_blob().cloned()).filter(|b| !b.is_empty()),
            checksum: Some(text(6)).filter(|s| !s.is_empty()),
            priority: row.get_value(7).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0) as i32,
            created_at: text(8), updated_at: text(9),
        });
    }
    Ok(out)
}

pub async fn delete(conn: &Connection, id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM deployment_configs WHERE id = ?1", [id]).await?;
    Ok(())
}
