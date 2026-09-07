//! CRUD for the `sessions` table.

use core::Session;
use turso::Connection;

pub async fn insert(conn: &Connection, s: &Session) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, tenant_id, token, ip_address, created_at, expires_at, \
         last_active) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        (
            s.id.as_str(), s.tenant_id.as_str(), s.token.as_str(),
            s.ip_address.as_deref().unwrap_or(""),
            s.created_at.as_str(), s.expires_at.as_str(), s.last_active.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn get_by_token(conn: &Connection, token: &str) -> anyhow::Result<Option<Session>> {
    let mut rows = conn.query("SELECT id, tenant_id, token, ip_address, created_at, expires_at, \
        last_active FROM sessions WHERE token = ?1", [token]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    Ok(Some(Session {
        id: text(0), tenant_id: text(1), token: text(2),
        ip_address: Some(text(3)).filter(|s| !s.is_empty()),
        created_at: text(4), expires_at: text(5), last_active: text(6),
    }))
}

pub async fn delete(conn: &Connection, token: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM sessions WHERE token = ?1", [token]).await?;
    Ok(())
}

pub async fn delete_by_tenant(conn: &Connection, tenant_id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM sessions WHERE tenant_id = ?1", [tenant_id]).await?;
    Ok(())
}
