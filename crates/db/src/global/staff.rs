//! CRUD for the `staff` table.

use serde::{Deserialize, Serialize};
use turso::Connection;

/// One row of the global `staff` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffRow {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
}

pub async fn create(conn: &Connection, s: &StaffRow) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO staff (id, username, password_hash, role, created_at) VALUES (?1,?2,?3,?4,?5)",
        (
            s.id.as_str(), s.username.as_str(), s.password_hash.as_str(),
            s.role.as_str(), s.created_at.as_str(),
        ),
    ).await?;
    Ok(())
}

fn row_to_staff(row: &turso::Row) -> StaffRow {
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    StaffRow {
        id: text(0), username: text(1), password_hash: text(2),
        role: text(3), created_at: text(4),
    }
}

pub async fn get_by_id(conn: &Connection, id: &str) -> anyhow::Result<Option<StaffRow>> {
    let mut rows = conn.query(
        "SELECT id, username, password_hash, role, created_at FROM staff WHERE id = ?1", [id],
    ).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to_staff(&row)))
}

pub async fn get_by_username(conn: &Connection, username: &str) -> anyhow::Result<Option<StaffRow>> {
    let mut rows = conn.query(
        "SELECT id, username, password_hash, role, created_at FROM staff WHERE username = ?1",
        [username],
    ).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    Ok(Some(row_to_staff(&row)))
}

pub async fn list(conn: &Connection) -> anyhow::Result<Vec<StaffRow>> {
    let mut rows = conn.query(
        "SELECT id, username, password_hash, role, created_at FROM staff ORDER BY username", (),
    ).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(row_to_staff(&row));
    }
    Ok(out)
}
