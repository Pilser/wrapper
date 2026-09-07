//! The `profiles` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub tenant_id: String,
    pub agent_name: String,
    pub nickname: Option<String>,
    pub language: String,
    pub timezone: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn upsert(conn: &Connection, p: &Profile) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO profiles (tenant_id, agent_name, nickname, language, timezone, created_at, \
         updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7) \
         ON CONFLICT(tenant_id) DO UPDATE SET agent_name=?2, nickname=?3, language=?4, timezone=?5, updated_at=?7",
        (
            p.tenant_id.as_str(), p.agent_name.as_str(), p.nickname.as_deref().unwrap_or(""),
            p.language.as_str(), p.timezone.as_str(), p.created_at.as_str(), p.updated_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn get(conn: &Connection, tenant_id: &str) -> anyhow::Result<Option<Profile>> {
    let mut rows = conn.query("SELECT tenant_id, agent_name, nickname, language, timezone, created_at, \
        updated_at FROM profiles WHERE tenant_id = ?1", [tenant_id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    Ok(Some(Profile {
        tenant_id: text(0), agent_name: text(1),
        nickname: Some(text(2)).filter(|s| !s.is_empty()),
        language: text(3), timezone: text(4), created_at: text(5), updated_at: text(6),
    }))
}
