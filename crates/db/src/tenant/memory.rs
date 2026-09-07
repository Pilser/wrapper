//! The `memory` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRow {
    pub id: String,
    pub key: String,
    pub value: String,
    pub category: String,
    pub importance: f64,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert(conn: &Connection, m: &MemoryRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO memory (id, key, value, category, importance, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (
            m.id.as_str(), m.key.as_str(), m.value.as_str(), m.category.as_str(),
            m.importance, m.created_at.as_str(), m.updated_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn get_by_key(conn: &Connection, key: &str) -> anyhow::Result<Option<MemoryRow>> {
    let mut rows = conn.query("SELECT id, key, value, category, importance, created_at, updated_at \
        FROM memory WHERE key = ?1", [key]).await?;
    Ok(rows_to_vec(&mut rows).await?.into_iter().next())
}

pub async fn list(conn: &Connection, category: Option<&str>, limit: Option<i64>) -> anyhow::Result<Vec<MemoryRow>> {
    let mut rows = match category {
        Some(c) => conn.query(
            "SELECT id, key, value, category, importance, created_at, updated_at \
             FROM memory WHERE category = ?1 ORDER BY importance DESC LIMIT ?2",
            (c, limit.unwrap_or(100))).await?,
        None => conn.query(
            "SELECT id, key, value, category, importance, created_at, updated_at \
             FROM memory ORDER BY importance DESC LIMIT ?1",
            [limit.unwrap_or(100)]).await?,
    };
    rows_to_vec(&mut rows).await
}

pub async fn search(conn: &Connection, q: &str) -> anyhow::Result<Vec<MemoryRow>> {
    let like = format!("%{q}%");
    let mut rows = conn.query(
        "SELECT id, key, value, category, importance, created_at, updated_at FROM memory \
         WHERE key LIKE ?1 OR value LIKE ?1 ORDER BY importance DESC",
        [like],
    ).await?;
    rows_to_vec(&mut rows).await
}

pub async fn delete(conn: &Connection, key: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM memory WHERE key = ?1", [key]).await?;
    Ok(())
}

async fn rows_to_vec(rows: &mut turso::Rows) -> anyhow::Result<Vec<MemoryRow>> {
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        let real = |i: usize| row.get_value(i).ok().and_then(|v| v.as_real().copied()).unwrap_or(0.5);
        out.push(MemoryRow {
            id: text(0), key: text(1), value: text(2), category: text(3),
            importance: real(4), created_at: text(5), updated_at: text(6),
        });
    }
    Ok(out)
}
