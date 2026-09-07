//! The `history` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRow {
    pub id: String,
    pub session_id: Option<String>,
    pub action: String,
    pub domain: Option<String>,
    pub summary: Option<String>,
    pub duration_seconds: Option<i64>,
    pub created_at: String,
}

pub async fn insert(conn: &Connection, h: &HistoryRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO history (id, session_id, action, domain, summary, duration_seconds, created_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (
            h.id.as_str(), h.session_id.as_deref().unwrap_or(""), h.action.as_str(),
            h.domain.as_deref().unwrap_or(""), h.summary.as_deref().unwrap_or(""),
            h.duration_seconds.unwrap_or(0), h.created_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection, domain: Option<&str>, limit: Option<i64>) -> anyhow::Result<Vec<HistoryRow>> {
    let mut rows = match domain {
        Some(d) => conn.query(
            "SELECT id, session_id, action, domain, summary, duration_seconds, created_at \
             FROM history WHERE domain = ?1 ORDER BY created_at DESC LIMIT ?2",
            (d, limit.unwrap_or(100))).await?,
        None => conn.query(
            "SELECT id, session_id, action, domain, summary, duration_seconds, created_at \
             FROM history ORDER BY created_at DESC LIMIT ?1",
            [limit.unwrap_or(100)]).await?,
    };
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        out.push(HistoryRow {
            id: text(0),
            session_id: Some(text(1)).filter(|s| !s.is_empty()),
            action: text(2),
            domain: Some(text(3)).filter(|s| !s.is_empty()),
            summary: Some(text(4)).filter(|s| !s.is_empty()),
            duration_seconds: row.get_value(5).ok().and_then(|v| v.as_integer().copied()),
            created_at: text(6),
        });
    }
    Ok(out)
}
