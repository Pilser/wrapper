//! The `configs` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRow {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

pub async fn set(conn: &Connection, key: &str, value: &str, now: &str) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO configs (key, value, updated_at) VALUES (?1,?2,?3) \
         ON CONFLICT(key) DO UPDATE SET value=?2, updated_at=?3",
        (key, value, now),
    ).await?;
    Ok(())
}

pub async fn get(conn: &Connection, key: &str) -> anyhow::Result<Option<ConfigRow>> {
    let mut rows = conn.query("SELECT id, key, value, updated_at FROM configs WHERE key = ?1", [key]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    let int = row.get_value(0).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    Ok(Some(ConfigRow { id: int, key: text(1), value: text(2), updated_at: text(3) }))
}
