//! The `launcher_scripts` table: guest launcher scripts per scope.
//!
//! Precedence per program: tenant row > category row > global row.
//! `name` is `zw` or `engram`. The supervisor demands these at boot (and
//! re-demands on repeated crashes); nothing is baked into the image.

use turso::Connection;

/// Program names the supervisor may request.
pub const NAMES: &[&str] = &["zw", "engram"];

/// Host filename per program inside the launchers dir.
pub fn host_filename(name: &str) -> String {
    format!("{name}.sh")
}

/// Seed global rows from `<dir>/{zw.sh,engram.sh}` when the global row is
/// empty. Never overwrites portal edits: only fills gaps. Returns the
/// number of rows seeded.
pub async fn seed_global_if_empty(
    conn: &Connection,
    dir: &std::path::Path,
    now: &str,
) -> anyhow::Result<usize> {
    let mut n = 0usize;
    for name in NAMES {
        if get(conn, "global", "", name).await?.is_some() {
            continue;
        }
        let p = dir.join(host_filename(name));
        let Ok(content) = std::fs::read_to_string(&p) else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }
        set(conn, "global", "", name, &content, now).await?;
        n += 1;
    }
    Ok(n)
}

pub async fn get(
    conn: &Connection,
    scope: &str,
    ref_id: &str,
    name: &str,
) -> anyhow::Result<Option<String>> {
    let mut rows = conn
        .query(
            "SELECT content FROM launcher_scripts WHERE scope = ?1 AND ref_id = ?2 AND name = ?3",
            (scope, ref_id, name),
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    Ok(row
        .get_value(0)
        .ok()
        .and_then(|v| v.as_text().cloned()))
}

pub async fn set(
    conn: &Connection,
    scope: &str,
    ref_id: &str,
    name: &str,
    content: &str,
    now: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO launcher_scripts (scope, ref_id, name, content, updated_at) VALUES (?1,?2,?3,?4,?5) \
         ON CONFLICT(scope, ref_id, name) DO UPDATE SET content=?4, updated_at=?5",
        (scope, ref_id, name, content, now),
    )
    .await?;
    Ok(())
}

/// Resolve one program for a tenant. Returns `(content, source)` where
/// source is `tenant:<id>`, `category:<id>` or `global`.
pub async fn effective(
    conn: &Connection,
    tenant_id: &str,
    category_id: &str,
    name: &str,
) -> anyhow::Result<(Option<String>, String)> {
    if let Some(c) = get(conn, "tenant", tenant_id, name).await? {
        return Ok((Some(c), format!("tenant:{tenant_id}")));
    }
    if !category_id.is_empty() {
        if let Some(c) = get(conn, "category", category_id, name).await? {
            return Ok((Some(c), format!("category:{category_id}")));
        }
    }
    if let Some(c) = get(conn, "global", "", name).await? {
        return Ok((Some(c), "global".to_string()));
    }
    Ok((None, "none".to_string()))
}
