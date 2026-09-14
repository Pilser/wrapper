//! The `attempts` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptRow {
    pub id: i64,
    pub task_id: i64,
    pub result: String,
    pub student_output: Option<String>,
    pub feedback: Option<String>,
    pub example_solution: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
}

pub async fn insert(conn: &Connection, task_id: i64, result: &str, student_output: Option<&str>, feedback: Option<&str>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO attempts (task_id, result, student_output, feedback, example_solution, created_at) VALUES (?1,?2,?3,?4,'',?5)",
        (task_id, result, student_output.unwrap_or(""), feedback.unwrap_or(""), now),
    ).await?;
    let id = conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0);
    // auto timeline
    super::timeline::insert(conn, "attempt", Some(id), &format!("attempt {result} for task {task_id}"), Some("attempt"), now).await.ok();
    Ok(id)
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<AttemptRow>> {
    let mut rows = conn.query("SELECT id, task_id, result, student_output, feedback, example_solution, resolved_at, created_at FROM attempts WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to(&row)))
}

pub async fn list_by_task(conn: &Connection, task_id: i64, limit: Option<i64>) -> anyhow::Result<Vec<AttemptRow>> {
    let mut rows = conn.query("SELECT id, task_id, result, student_output, feedback, example_solution, resolved_at, created_at FROM attempts WHERE task_id = ?1 ORDER BY id DESC LIMIT ?2", (task_id, limit.unwrap_or(100))).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? { out.push(row_to(&row)); }
    Ok(out)
}

pub async fn resolve(conn: &Connection, id: i64, example: &str, note: Option<&str>, now: &str) -> anyhow::Result<()> {
    let feedback_extra = note.unwrap_or("");
    conn.execute("UPDATE attempts SET example_solution = ?1, feedback = COALESCE(feedback,'') || ?2, resolved_at = ?3 WHERE id = ?4", (example, if feedback_extra.is_empty(){ "".to_string()} else { format!(" | note: {feedback_extra}") }, now, id)).await?;
    super::timeline::insert(conn, "attempt_resolve", Some(id), &format!("resolved attempt {id}"), Some("attempt"), now).await.ok();
    Ok(())
}

fn row_to(row: &turso::Row) -> AttemptRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    AttemptRow { id: int(0), task_id: int(1), result: text(2), student_output: Some(text(3)).filter(|s| !s.is_empty()), feedback: Some(text(4)).filter(|s| !s.is_empty()), example_solution: Some(text(5)).filter(|s| !s.is_empty()), resolved_at: Some(text(6)).filter(|s| !s.is_empty()), created_at: text(7) }
}
