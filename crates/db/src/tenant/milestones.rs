//! The `milestones` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneRow {
    pub id: i64,
    pub goal_id: i64,
    pub title: String,
    pub order_index: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert(conn: &Connection, goal_id: i64, title: &str, order_index: Option<i64>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO milestones (goal_id, title, order_index, status, created_at, updated_at) VALUES (?1,?2,?3,'open',?4,?4)",
        (goal_id, title, order_index.unwrap_or(0), now),
    ).await?;
    Ok(conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0))
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<MilestoneRow>> {
    let mut rows = conn.query("SELECT id, goal_id, title, order_index, status, created_at, updated_at FROM milestones WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to(&row)))
}

pub async fn list_by_goal(conn: &Connection, goal_id: i64) -> anyhow::Result<Vec<MilestoneRow>> {
    let mut rows = conn.query("SELECT id, goal_id, title, order_index, status, created_at, updated_at FROM milestones WHERE goal_id = ?1 ORDER BY order_index, id", [goal_id]).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? { out.push(row_to(&row)); }
    Ok(out)
}

pub async fn update(conn: &Connection, id: i64, title: Option<&str>, status: Option<&str>, now: &str) -> anyhow::Result<()> {
    if let Some(t) = title { conn.execute("UPDATE milestones SET title = ?1, updated_at = ?2 WHERE id = ?3", (t, now, id)).await?; }
    if let Some(s) = status { conn.execute("UPDATE milestones SET status = ?1, updated_at = ?2 WHERE id = ?3", (s, now, id)).await?; }
    Ok(())
}

fn row_to(row: &turso::Row) -> MilestoneRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    MilestoneRow { id: int(0), goal_id: int(1), title: text(2), order_index: int(3), status: text(4), created_at: text(5), updated_at: text(6) }
}
