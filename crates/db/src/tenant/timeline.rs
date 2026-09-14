//! The `timeline` table — append-only diary.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineRow {
    pub id: i64,
    pub kind: String,
    pub ref_id: Option<i64>,
    pub summary: String,
    pub ref_type: Option<String>,
    pub created_at: String,
}

pub async fn insert(conn: &Connection, kind: &str, ref_id: Option<i64>, summary: &str, ref_type: Option<&str>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO timeline (kind, ref_id, summary, ref_type, created_at) VALUES (?1,?2,?3,?4,?5)",
        (kind, ref_id.unwrap_or(0), summary, ref_type.unwrap_or(""), now),
    ).await?;
    Ok(conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0))
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<TimelineRow>> {
    let mut rows = conn.query("SELECT id, kind, ref_id, summary, ref_type, created_at FROM timeline WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to(&row)))
}

pub async fn list(conn: &Connection, kind: Option<&str>, since: Option<&str>, limit: Option<i64>) -> anyhow::Result<Vec<TimelineRow>> {
    let mut sql = String::from("SELECT id, kind, ref_id, summary, ref_type, created_at FROM timeline WHERE 1=1");
    if let Some(k) = kind { sql.push_str(&format!(" AND kind = '{}'", k.replace('\'',"''"))); }
    if let Some(s) = since { sql.push_str(&format!(" AND created_at >= '{}'", s.replace('\'',"''"))); }
    sql.push_str(" ORDER BY id DESC LIMIT ");
    sql.push_str(&limit.unwrap_or(50).to_string());
    let mut rows = conn.query(&sql, ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? { out.push(row_to(&row)); }
    Ok(out)
}

fn row_to(row: &turso::Row) -> TimelineRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    let ref_id = int(2);
    TimelineRow { id: int(0), kind: text(1), ref_id: if ref_id==0 { None } else { Some(ref_id) }, summary: text(3), ref_type: Some(text(4)).filter(|s| !s.is_empty()), created_at: text(5) }
}
