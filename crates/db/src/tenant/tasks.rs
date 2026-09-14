//! The `tasks` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRow {
    pub id: i64,
    pub milestone_id: i64,
    pub title: String,
    pub kind: String,
    pub description: Option<String>,
    pub difficulty: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert(conn: &Connection, milestone_id: i64, title: &str, kind: &str, description: Option<&str>, difficulty: Option<&str>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO tasks (milestone_id, title, kind, description, difficulty, status, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,'open',?6,?6)",
        (milestone_id, title, kind, description.unwrap_or(""), difficulty.unwrap_or("medium"), now),
    ).await?;
    Ok(conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0))
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<TaskRow>> {
    let mut rows = conn.query("SELECT id, milestone_id, title, kind, description, difficulty, status, created_at, updated_at FROM tasks WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to(&row)))
}

pub async fn list(conn: &Connection, milestone_id: Option<i64>, status: Option<&str>, kind: Option<&str>) -> anyhow::Result<Vec<TaskRow>> {
    let mut sql = String::from("SELECT id, milestone_id, title, kind, description, difficulty, status, created_at, updated_at FROM tasks WHERE 1=1");
    let mut args: Vec<String> = Vec::new();
    if let Some(m) = milestone_id { sql.push_str(&format!(" AND milestone_id = {}", m)); }
    if let Some(s) = status { sql.push_str(&format!(" AND status = '{}'", s.replace('\'',"''"))); let _ = args; }
    if let Some(k) = kind { sql.push_str(&format!(" AND kind = '{}'", k.replace('\'',"''"))); }
    sql.push_str(" ORDER BY id DESC LIMIT 100");
    let mut rows = conn.query(&sql, ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? { out.push(row_to(&row)); }
    Ok(out)
}

pub async fn update(conn: &Connection, id: i64, title: Option<&str>, status: Option<&str>, description: Option<&str>, now: &str) -> anyhow::Result<()> {
    if let Some(t) = title { conn.execute("UPDATE tasks SET title = ?1, updated_at = ?2 WHERE id = ?3", (t, now, id)).await?; }
    if let Some(s) = status { conn.execute("UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3", (s, now, id)).await?; }
    if let Some(d) = description { conn.execute("UPDATE tasks SET description = ?1, updated_at = ?2 WHERE id = ?3", (d, now, id)).await?; }
    Ok(())
}

fn row_to(row: &turso::Row) -> TaskRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    TaskRow { id: int(0), milestone_id: int(1), title: text(2), kind: text(3), description: Some(text(4)).filter(|s| !s.is_empty()), difficulty: text(5), status: text(6), created_at: text(7), updated_at: text(8) }
}
