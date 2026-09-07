//! CRUD for the `tenants` table.

use shared::Tenant;
use telemetry::{Level, Telemetry};
use turso::Connection;

pub async fn create(conn: &Connection, t: &Tenant, tel: &Telemetry) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO tenants (id, username, password_hash, display_name, category_id, vm_ip, \
         vm_process_id, vm_template_id, tap_device, status, db_path, created_at, last_login, \
         category_changed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        (
            t.id.as_str(), t.username.as_str(), t.password_hash.as_str(),
            t.display_name.as_deref().unwrap_or(""), t.category_id.as_str(),
            t.vm_ip.as_deref().unwrap_or(""), t.vm_process_id.unwrap_or(0),
            t.vm_template_id.as_deref().unwrap_or(""), t.tap_device.as_deref().unwrap_or(""),
            t.status.as_str(), t.db_path.as_str(), t.created_at.as_str(),
            t.last_login.as_deref().unwrap_or(""), t.category_changed_at.as_deref().unwrap_or(""),
        ),
    )
    .await?;
    tel.builder().level(Level::Info).kind("db.write.completed").category("db")
        .message(format!("tenant {} created", t.id)).tenant_id(Some(t.id.clone())).emit();
    Ok(())
}

pub async fn get_by_id(conn: &Connection, id: &str) -> anyhow::Result<Option<Tenant>> {
    let mut rows = conn.query("SELECT id, username, password_hash, display_name, category_id, \
        vm_ip, vm_process_id, vm_template_id, tap_device, status, db_path, created_at, \
        last_login, category_changed_at FROM tenants WHERE id = ?1", [id]).await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    Ok(Some(Tenant {
        id: text(0), username: text(1), password_hash: text(2),
        display_name: Some(text(3)).filter(|s| !s.is_empty()),
        category_id: text(4),
        vm_ip: Some(text(5)).filter(|s| !s.is_empty()),
        vm_process_id: row.get_value(6).ok().and_then(|v| v.as_integer().copied()),
        vm_template_id: Some(text(7)).filter(|s| !s.is_empty()),
        tap_device: Some(text(8)).filter(|s| !s.is_empty()),
        status: shared::TenantStatus::Active,
        db_path: text(10), created_at: text(11),
        last_login: Some(text(12)).filter(|s| !s.is_empty()),
        category_changed_at: Some(text(13)).filter(|s| !s.is_empty()),
    }))
}

fn parse_status(s: &str) -> shared::TenantStatus {
    match s {
        "suspended" => shared::TenantStatus::Suspended,
        "booting" => shared::TenantStatus::Booting,
        "ready" => shared::TenantStatus::Ready,
        "asleep" => shared::TenantStatus::Asleep,
        "deleted" => shared::TenantStatus::Deleted,
        _ => shared::TenantStatus::Active,
    }
}

pub async fn update(conn: &Connection, t: &Tenant) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE tenants SET username = ?1, password_hash = ?2, display_name = ?3, category_id = ?4, \
         vm_ip = ?5, vm_process_id = ?6, vm_template_id = ?7, tap_device = ?8, status = ?9, \
         db_path = ?10, last_login = ?11, category_changed_at = ?12 WHERE id = ?13",
        (
            t.username.as_str(), t.password_hash.as_str(),
            t.display_name.as_deref().unwrap_or(""), t.category_id.as_str(),
            t.vm_ip.as_deref().unwrap_or(""), t.vm_process_id.unwrap_or(0),
            t.vm_template_id.as_deref().unwrap_or(""), t.tap_device.as_deref().unwrap_or(""),
            t.status.as_str(), t.db_path.as_str(),
            t.last_login.as_deref().unwrap_or(""), t.category_changed_at.as_deref().unwrap_or(""),
            t.id.as_str(),
        ),
    )
    .await?;
    Ok(())
}

pub async fn delete(conn: &Connection, id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM tenants WHERE id = ?1", [id]).await?;
    Ok(())
}

pub async fn list(conn: &Connection) -> anyhow::Result<Vec<Tenant>> {
    let mut rows = conn.query("SELECT id, username, password_hash, display_name, category_id, \
        vm_ip, vm_process_id, vm_template_id, tap_device, status, db_path, created_at, \
        last_login, category_changed_at FROM tenants ORDER BY username", ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        out.push(Tenant {
            id: text(0), username: text(1), password_hash: text(2),
            display_name: Some(text(3)).filter(|s| !s.is_empty()),
            category_id: text(4),
            vm_ip: Some(text(5)).filter(|s| !s.is_empty()),
            vm_process_id: row.get_value(6).ok().and_then(|v| v.as_integer().copied()),
            vm_template_id: Some(text(7)).filter(|s| !s.is_empty()),
            tap_device: Some(text(8)).filter(|s| !s.is_empty()),
            status: parse_status(&text(9)),
            db_path: text(10), created_at: text(11),
            last_login: Some(text(12)).filter(|s| !s.is_empty()),
            category_changed_at: Some(text(13)).filter(|s| !s.is_empty()),
        });
    }
    Ok(out)
}
