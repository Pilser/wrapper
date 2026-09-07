//! The `projects` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert(conn: &Connection, p: &ProjectRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO projects (id, name, domain, status, description, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (
            p.id.as_str(), p.name.as_str(), p.domain.as_deref().unwrap_or(""),
            p.status.as_str(), p.description.as_deref().unwrap_or(""),
            p.created_at.as_str(), p.updated_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection, status: Option<&str>) -> anyhow::Result<Vec<ProjectRow>> {
    let mut rows = match status {
        Some(s) => conn.query("SELECT id, name, domain, status, description, created_at, updated_at \
            FROM projects WHERE status = ?1", [s]).await?,
        None => conn.query("SELECT id, name, domain, status, description, created_at, updated_at \
            FROM projects", ()).await?,
    };
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        out.push(ProjectRow {
            id: text(0), name: text(1),
            domain: Some(text(2)).filter(|s| !s.is_empty()),
            status: text(3),
            description: Some(text(4)).filter(|s| !s.is_empty()),
            created_at: text(5), updated_at: text(6),
        });
    }
    Ok(out)
}
