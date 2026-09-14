//! The `goals` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalRow {
    pub id: i64,
    pub title: String,
    pub track: Option<String>,
    pub target_date: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert(conn: &Connection, title: &str, track: Option<&str>, target_date: Option<&str>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO goals (title, track, target_date, status, created_at, updated_at) VALUES (?1,?2,?3,'active',?4,?4)",
        (title, track.unwrap_or(""), target_date.unwrap_or(""), now),
    ).await?;
    Ok(conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0))
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<GoalRow>> {
    let mut rows = conn.query("SELECT id, title, track, target_date, status, created_at, updated_at FROM goals WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to_goal(&row)))
}

pub async fn list(conn: &Connection, status: Option<&str>, limit: Option<i64>) -> anyhow::Result<Vec<GoalRow>> {
    let mut rows = if let Some(s) = status {
        conn.query("SELECT id, title, track, target_date, status, created_at, updated_at FROM goals WHERE status = ?1 ORDER BY id DESC LIMIT ?2", (s, limit.unwrap_or(100))).await?
    } else {
        conn.query("SELECT id, title, track, target_date, status, created_at, updated_at FROM goals ORDER BY id DESC LIMIT ?1", [limit.unwrap_or(100)]).await?
    };
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? { out.push(row_to_goal(&row)); }
    Ok(out)
}

pub async fn update(conn: &Connection, id: i64, title: Option<&str>, status: Option<&str>, target_date: Option<&str>, now: &str) -> anyhow::Result<()> {
    if let Some(t) = title {
        conn.execute("UPDATE goals SET title = ?1, updated_at = ?2 WHERE id = ?3", (t, now, id)).await?;
    }
    if let Some(s) = status {
        conn.execute("UPDATE goals SET status = ?1, updated_at = ?2 WHERE id = ?3", (s, now, id)).await?;
    }
    if let Some(d) = target_date {
        conn.execute("UPDATE goals SET target_date = ?1, updated_at = ?2 WHERE id = ?3", (d, now, id)).await?;
    }
    Ok(())
}

fn row_to_goal(row: &turso::Row) -> GoalRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let id = row.get_value(0).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    GoalRow { id, title: text(1), track: Some(text(2)).filter(|s| !s.is_empty()), target_date: Some(text(3)).filter(|s| !s.is_empty()), status: text(4), created_at: text(5), updated_at: text(6) }
}
