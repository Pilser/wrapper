//! CRUD for the `networks` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

/// A reserved IP for a tenant (row in `networks`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetReservation {
    pub tenant_id: String,
    pub ip_address: String,
    pub tap_device: String,
}

pub async fn reserve(conn: &Connection, tenant_id: &str, ip: &str, tap: &str) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO networks (id, tenant_id, ip_address, tap_device, allocated_at) \
         VALUES (?1,?2,?3,?4,?5)",
        (format!("net_{tenant_id}"), tenant_id, ip, tap, crate::now_str()),
    ).await?;
    Ok(())
}

pub async fn release_all(conn: &Connection, tenant_id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM networks WHERE tenant_id = ?1", [tenant_id]).await?;
    Ok(())
}

pub async fn all(conn: &Connection) -> anyhow::Result<Vec<NetReservation>> {
    let mut rows = conn.query("SELECT tenant_id, ip_address, tap_device FROM networks", ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let g = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        out.push(NetReservation { tenant_id: g(0), ip_address: g(1), tap_device: g(2) });
    }
    Ok(out)
}
