//! The `reports` table — portfolio snapshots.
use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRow {
    pub id: i64,
    pub period_start: String,
    pub period_end: String,
    pub content: String,
    pub highlights: Option<String>,
    pub gaps: Option<String>,
    pub next_steps: Option<String>,
    pub generated_at: String,
}

pub async fn insert(conn: &Connection, period_start: &str, period_end: &str, content: &str, highlights: Option<&str>, gaps: Option<&str>, next_steps: Option<&str>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO reports (period_start, period_end, content, highlights, gaps, next_steps, generated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (period_start, period_end, content, highlights.unwrap_or(""), gaps.unwrap_or(""), next_steps.unwrap_or(""), now),
    ).await?;
    Ok(conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0))
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<ReportRow>> {
    let mut rows = conn.query("SELECT id, period_start, period_end, content, highlights, gaps, next_steps, generated_at FROM reports WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to(&row)))
}

pub async fn list(conn: &Connection, limit: Option<i64>) -> anyhow::Result<Vec<ReportRow>> {
    let mut rows = conn.query("SELECT id, period_start, period_end, content, highlights, gaps, next_steps, generated_at FROM reports ORDER BY generated_at DESC LIMIT ?1", [limit.unwrap_or(20)]).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? { out.push(row_to(&row)); }
    Ok(out)
}

fn row_to(row: &turso::Row) -> ReportRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    ReportRow { id: int(0), period_start: text(1), period_end: text(2), content: text(3), highlights: Some(text(4)).filter(|s| !s.is_empty()), gaps: Some(text(5)).filter(|s| !s.is_empty()), next_steps: Some(text(6)).filter(|s| !s.is_empty()), generated_at: text(7) }
}
