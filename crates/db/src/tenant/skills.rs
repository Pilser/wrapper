//! The `skills` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub domain: Option<String>,
    pub level: String,
    pub confidence: f64,
    pub last_practiced: Option<String>,
    pub created_at: String,
}

pub async fn insert(conn: &Connection, s: &SkillRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO skills (id, name, domain, level, confidence, last_practiced, created_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (
            s.id.as_str(), s.name.as_str(), s.domain.as_deref().unwrap_or(""),
            s.level.as_str(), s.confidence, s.last_practiced.as_deref().unwrap_or(""),
            s.created_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection, domain: Option<&str>) -> anyhow::Result<Vec<SkillRow>> {
    let mut rows = match domain {
        Some(d) => conn.query("SELECT id, name, domain, level, confidence, last_practiced, created_at \
            FROM skills WHERE domain = ?1 ORDER BY confidence DESC", [d]).await?,
        None => conn.query("SELECT id, name, domain, level, confidence, last_practiced, created_at \
            FROM skills ORDER BY confidence DESC", ()).await?,
    };
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        let real = |i: usize| row.get_value(i).ok().and_then(|v| v.as_real().copied()).unwrap_or(0.5);
        out.push(SkillRow {
            id: text(0), name: text(1),
            domain: Some(text(2)).filter(|s| !s.is_empty()),
            level: text(3), confidence: real(4),
            last_practiced: Some(text(5)).filter(|s| !s.is_empty()),
            created_at: text(6),
        });
    }
    Ok(out)
}
