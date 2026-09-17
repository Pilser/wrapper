//! CRUD for `binary_versions` + `binary_active` (Binaries tab history).
//!
//! Keep at most 5 rows per bundle; the active (in-play) row is never pruned.
//! Rollback only moves the pointer — it never inserts a row.

use serde::{Deserialize, Serialize};
use turso::Connection;

/// One committed binary version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryVersion {
    pub id: String,
    pub name: String,
    pub version_no: i64,
    pub uploaded_at: String,
    pub merged_at: String,
    pub created_by: String,
    pub manifest_json: String,
    pub note: String,
}

fn row(r: &turso::Row) -> BinaryVersion {
    let text = |i: usize| {
        r.get_value(i)
            .ok()
            .and_then(|v| v.as_text().cloned())
            .unwrap_or_default()
    };
    BinaryVersion {
        id: text(0),
        name: text(1),
        version_no: r.get_value(2).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0),
        uploaded_at: text(3),
        merged_at: text(4),
        created_by: text(5),
        manifest_json: text(6),
        note: text(7),
    }
}

/// Next version number for a bundle (monotonic, never reused).
pub async fn next_no(conn: &Connection, name: &str) -> anyhow::Result<i64> {
    let mut rows = conn
        .query(
            "SELECT COALESCE(MAX(version_no),0) FROM binary_versions WHERE name = ?1",
            [name],
        )
        .await?;
    Ok(rows
        .next()
        .await?
        .and_then(|r| r.get_value(0).ok()?.as_integer().copied())
        .unwrap_or(0)
        + 1)
}

pub async fn insert(
    conn: &Connection,
    name: &str,
    version_no: i64,
    uploaded_at: &str,
    merged_at: &str,
    created_by: &str,
    manifest_json: &str,
    note: &str,
) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO binary_versions (id, name, version_no, uploaded_at, merged_at, created_by, manifest_json, note) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        (
            id.as_str(), name, version_no, uploaded_at, merged_at, created_by, manifest_json, note,
        ),
    )
    .await?;
    Ok(id)
}

/// Newest-first version rows for a bundle (empty name = all bundles).
pub async fn list(conn: &Connection, name: &str) -> anyhow::Result<Vec<BinaryVersion>> {
    const COLS: &str = "id, name, version_no, uploaded_at, merged_at, created_by, manifest_json, note";
    let mut out = Vec::new();
    if name.is_empty() {
        let mut q = conn
            .query(
                &format!("SELECT {COLS} FROM binary_versions ORDER BY name, version_no DESC"),
                (),
            )
            .await?;
        while let Some(r) = q.next().await? {
            out.push(row(&r));
        }
    } else {
        let mut q = conn
            .query(
                &format!("SELECT {COLS} FROM binary_versions WHERE name = ?1 ORDER BY version_no DESC"),
                [name],
            )
            .await?;
        while let Some(r) = q.next().await? {
            out.push(row(&r));
        }
    }
    Ok(out)
}

pub async fn get(conn: &Connection, name: &str, version_no: i64) -> anyhow::Result<Option<BinaryVersion>> {
    let mut rows = conn
        .query(
            "SELECT id, name, version_no, uploaded_at, merged_at, created_by, manifest_json, note \
             FROM binary_versions WHERE name = ?1 AND version_no = ?2",
            (name, version_no),
        )
        .await?;
    Ok(rows.next().await?.map(|r| row(&r)))
}

/// Delete oldest non-active rows beyond 5 kept per bundle. Returns pruned count.
pub async fn prune(conn: &Connection, name: &str) -> anyhow::Result<i64> {
    let active = active_get(conn, name).await?;
    let rows = list(conn, name).await?;
    if rows.len() <= 5 {
        return Ok(0);
    }
    let mut pruned = 0i64;
    for r in rows.iter().skip(5) {
        if Some(r.version_no) == active {
            continue; // the in-play version is never pruned
        }
        conn.execute(
            "DELETE FROM binary_versions WHERE id = ?1",
            [r.id.as_str()],
        )
        .await?;
        pruned += 1;
    }
    Ok(pruned)
}

/// In-play version pointer (None = never versioned / live predates history).
pub async fn active_get(conn: &Connection, name: &str) -> anyhow::Result<Option<i64>> {
    let mut rows = conn
        .query("SELECT version_no FROM binary_active WHERE name = ?1", [name])
        .await?;
    Ok(rows
        .next()
        .await?
        .and_then(|r| r.get_value(0).ok()?.as_integer().copied()))
}

pub async fn active_set(conn: &Connection, name: &str, version_no: i64, now: &str) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO binary_active (name, version_no, activated_at) VALUES (?1,?2,?3)",
        (name, version_no, now),
    )
    .await?;
    Ok(())
}
