//! The `reports` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRow {
    pub id: String,
    pub period_start: String,
    pub period_end: String,
    pub content: String,
    pub highlights: Option<String>,
    pub areas_to_improve: Option<String>,
    pub generated_at: String,
}

pub async fn insert(conn: &Connection, r: &ReportRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO reports (id, period_start, period_end, content, highlights, areas_to_improve, generated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (
            r.id.as_str(), r.period_start.as_str(), r.period_end.as_str(), r.content.as_str(),
            r.highlights.as_deref().unwrap_or(""), r.areas_to_improve.as_deref().unwrap_or(""),
            r.generated_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection, since: Option<&str>, until: Option<&str>) -> anyhow::Result<Vec<ReportRow>> {
    let mut rows = conn.query(
        "SELECT id, period_start, period_end, content, highlights, areas_to_improve, generated_at \
         FROM reports ORDER BY generated_at DESC",
        (),
    ).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        if let Some(s) = since {
            if text(1).as_str() < s { continue; }
        }
        if let Some(u) = until {
            if text(2).as_str() > u { continue; }
        }
        out.push(ReportRow {
            id: text(0), period_start: text(1), period_end: text(2), content: text(3),
            highlights: Some(text(4)).filter(|s| !s.is_empty()),
            areas_to_improve: Some(text(5)).filter(|s| !s.is_empty()),
            generated_at: text(6),
        });
    }
    Ok(out)
}
