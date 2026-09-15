//! The `system_prompts` table: zw agent system prompt per scope.
//!
//! Precedence when resolving for a tenant: tenant row > category row >
//! global row > host `deployment/zerowrapper-mono/SYSTEM.md` file.

use turso::Connection;

pub async fn get(conn: &Connection, scope: &str, ref_id: &str) -> anyhow::Result<Option<String>> {
    let mut rows = conn
        .query(
            "SELECT content FROM system_prompts WHERE scope = ?1 AND ref_id = ?2",
            (scope, ref_id),
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

pub async fn set(conn: &Connection, scope: &str, ref_id: &str, content: &str, now: &str) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO system_prompts (scope, ref_id, content, updated_at) VALUES (?1,?2,?3,?4) \
         ON CONFLICT(scope, ref_id) DO UPDATE SET content=?3, updated_at=?4",
        (scope, ref_id, content, now),
    )
    .await?;
    Ok(())
}

/// Resolve the effective prompt for a tenant. Returns `(content, source)`
/// where source is `tenant:<id>`, `category:<id>`, `global` or `host-file`.
pub async fn effective(
    conn: &Connection,
    tenant_id: &str,
    category_id: &str,
) -> anyhow::Result<(Option<String>, String)> {
    if let Some(c) = get(conn, "tenant", tenant_id).await? {
        return Ok((Some(c), format!("tenant:{tenant_id}")));
    }
    if !category_id.is_empty() {
        if let Some(c) = get(conn, "category", category_id).await? {
            return Ok((Some(c), format!("category:{category_id}")));
        }
    }
    if let Some(c) = get(conn, "global", "").await? {
        return Ok((Some(c), "global".to_string()));
    }
    Ok((None, "host-file".to_string()))
}
