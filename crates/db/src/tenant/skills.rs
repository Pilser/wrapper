//! The `skills` table — portfolio skills.
use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRow {
    pub id: i64,
    pub name: String,
    pub domain: Option<String>,
    pub level: String,
    pub evidence: Option<i64>,
    pub confidence: f64,
    pub last_practiced: Option<String>,
    pub created_at: String,
}

pub async fn insert(conn: &Connection, name: &str, domain: Option<&str>, level: Option<&str>, evidence: Option<i64>, confidence: Option<f64>, now: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO skills (name, domain, level, evidence, confidence, last_practiced, created_at) VALUES (?1,?2,?3,?4,?5,?6,?6)",
        (name, domain.unwrap_or(""), level.unwrap_or("novice"), evidence.unwrap_or(0), confidence.unwrap_or(0.5), now),
    ).await?;
    let id = conn.query("SELECT last_insert_rowid()", ()).await?.next().await?.unwrap().get_value(0)?.as_integer().copied().unwrap_or(0);
    super::timeline::insert(conn, "skill", Some(id), &format!("skill recorded: {name}"), Some("skill"), now).await.ok();
    Ok(id)
}

pub async fn get(conn: &Connection, id: i64) -> anyhow::Result<Option<SkillRow>> {
    let mut rows = conn.query("SELECT id, name, domain, level, evidence, confidence, last_practiced, created_at FROM skills WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to(&row)))
}

pub async fn list(conn: &Connection, domain: Option<&str>) -> anyhow::Result<Vec<SkillRow>> {
    let mut rows = if let Some(d) = domain {
        conn.query("SELECT id, name, domain, level, evidence, confidence, last_practiced, created_at FROM skills WHERE domain = ?1 ORDER BY confidence DESC", [d]).await?
    } else {
        conn.query("SELECT id, name, domain, level, evidence, confidence, last_practiced, created_at FROM skills ORDER BY confidence DESC", ()).await?
    };
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? { out.push(row_to(&row)); }
    Ok(out)
}

pub async fn update(conn: &Connection, id: i64, level: Option<&str>, evidence: Option<i64>, now: &str) -> anyhow::Result<()> {
    if let Some(l) = level { conn.execute("UPDATE skills SET level = ?1, last_practiced = ?2 WHERE id = ?3", (l, now, id)).await?; }
    if let Some(e) = evidence { conn.execute("UPDATE skills SET evidence = ?1, last_practiced = ?2 WHERE id = ?3", (e, now, id)).await?; }
    Ok(())
}

fn row_to(row: &turso::Row) -> SkillRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let int = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    let real = |i: usize| row.get_value(i).ok().and_then(|v| v.as_real().copied()).unwrap_or(0.5);
    let ev = int(4);
    SkillRow { id: int(0), name: text(1), domain: Some(text(2)).filter(|s| !s.is_empty()), level: text(3), evidence: if ev==0 { None } else { Some(ev) }, confidence: real(5), last_practiced: Some(text(6)).filter(|s| !s.is_empty()), created_at: text(7) }
}
