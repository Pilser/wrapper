//! The `achievements` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementRow {
    pub id: i64,
    pub title: String,
    pub skill_id: Option<i64>,
    pub task_id: Option<i64>,
    pub awarded_at: String,
}

pub async fn insert(conn: &Connection, title: &str, skill_id: Option<i64>, task_id: Option<i64>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO achievements (title, skill_id, task_id, awarded_at) VALUES (?1,?2,?3,?4)",
        (title, skill_id.unwrap_or(0), task_id.unwrap_or(0), now),
    ).await?;
    let id = conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0);
    super::timeline::insert(conn, "achievement", Some(id), &format!("achievement: {title}"), Some("achievement"), now).await.ok();
    Ok(id)
}

pub async fn list(conn: &Connection, limit: Option<i64>) -> anyhow::Result<Vec<AchievementRow>> {
    let mut rows = conn.query("SELECT id, title, skill_id, task_id, awarded_at FROM achievements ORDER BY id DESC LIMIT ?1", [limit.unwrap_or(100)]).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
        let sid = int(2); let tid = int(3);
        out.push(AchievementRow { id: int(0), title: text(1), skill_id: if sid==0 { None } else { Some(sid) }, task_id: if tid==0 { None } else { Some(tid) }, awarded_at: text(4) });
    }
    Ok(out)
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<AchievementRow>> {
    let mut rows = conn.query("SELECT id, title, skill_id, task_id, awarded_at FROM achievements WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    let sid = int(2); let tid = int(3);
    Ok(Some(AchievementRow { id: int(0), title: text(1), skill_id: if sid==0 { None } else { Some(sid) }, task_id: if tid==0 { None } else { Some(tid) }, awarded_at: text(4) }))
}
