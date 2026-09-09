//! CRUD for the `vm_templates` table.

use shared::VmTemplate;
use turso::Connection;

pub async fn insert(conn: &Connection, t: &VmTemplate) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO vm_templates (id, name, description, rootfs_path, kernel_path, initrd_path, \
         vcpu_count, memory_mb, requires_gpu, status, created_at, updated_at) VALUES \
         (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        (
            t.id.as_str(), t.name.as_str(), t.description.as_deref().unwrap_or(""),
            t.rootfs_path.as_str(), t.kernel_path.as_str(), t.initrd_path.as_str(),
            t.vcpu_count as i64, t.memory_mb as i64, t.requires_gpu as i64,
            t.status.as_str(), t.created_at.as_str(), t.updated_at.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn list(conn: &Connection) -> anyhow::Result<Vec<VmTemplate>> {
    let mut rows = conn.query("SELECT id, name, description, rootfs_path, kernel_path, initrd_path, \
        vcpu_count, memory_mb, requires_gpu, status, created_at, updated_at FROM vm_templates", ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
        let integer = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
        out.push(VmTemplate {
            id: text(0), name: text(1),
            description: Some(text(2)).filter(|s| !s.is_empty()),
            rootfs_path: text(3), kernel_path: text(4), initrd_path: text(5),
            vcpu_count: integer(6) as u32, memory_mb: integer(7) as u32,
            requires_gpu: integer(8) != 0,
            status: parse_status(&text(9)),
            created_at: text(10), updated_at: text(11),
        });
    }
    Ok(out)
}

pub async fn update(conn: &Connection, t: &VmTemplate) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE vm_templates SET name = ?1, description = ?2, rootfs_path = ?3, kernel_path = ?4, \
         initrd_path = ?5, vcpu_count = ?6, memory_mb = ?7, requires_gpu = ?8, status = ?9, \
         updated_at = ?10 WHERE id = ?11",
        (
            t.name.as_str(), t.description.as_deref().unwrap_or(""),
            t.rootfs_path.as_str(), t.kernel_path.as_str(), t.initrd_path.as_str(),
            t.vcpu_count as i64, t.memory_mb as i64, t.requires_gpu as i64,
            t.status.as_str(), t.updated_at.as_str(), t.id.as_str(),
        ),
    ).await?;
    Ok(())
}

pub async fn get_by_id(conn: &Connection, id: &str) -> anyhow::Result<Option<VmTemplate>> {
    let mut rows = conn.query(
        "SELECT id, name, description, rootfs_path, kernel_path, initrd_path, vcpu_count, \
         memory_mb, requires_gpu, status, created_at, updated_at FROM vm_templates WHERE id = ?1",
        [id],
    )
    .await?;
    let Some(row) = rows.next().await? else { return Ok(None) };
    let text = |i: usize| row.get_value(i).ok().and_then(|v| v.as_text().cloned()).unwrap_or_default();
    let integer = |i: usize| row.get_value(i).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0);
    Ok(Some(VmTemplate {
        id: text(0), name: text(1),
        description: Some(text(2)).filter(|s| !s.is_empty()),
        rootfs_path: text(3), kernel_path: text(4), initrd_path: text(5),
            vcpu_count: integer(6) as u32, memory_mb: integer(7) as u32,
            requires_gpu: integer(8) != 0,
            status: parse_status(&text(9)),
            created_at: text(10), updated_at: text(11),
        }))
    }

fn parse_status(s: &str) -> shared::VmTemplateStatus {
    match s {
        "retired" => shared::VmTemplateStatus::Retired,
        _ => shared::VmTemplateStatus::Active,
    }
}

pub async fn delete(conn: &Connection, id: &str) -> anyhow::Result<()> {
    conn.execute("DELETE FROM vm_templates WHERE id = ?1", [id]).await?;
    Ok(())
}
