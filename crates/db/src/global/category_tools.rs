//! Per-category MCP tool grants for in-VM supervisor tools.
//!
//! Semantics: no rows for a category = every tool allowed (backward
//! compatible). One or more rows = exactly those dotted op names
//! (`memory.save`, …) are listed and callable for tenants of that category.

use turso::Connection;

use shared::now_rfc3339;

/// Granted tool op names for a category, in sorted order. Empty = unrestricted.
pub async fn list(conn: &Connection, category_id: &str) -> anyhow::Result<Vec<String>> {
    let mut rows = conn
        .query(
            "SELECT tool FROM category_tools WHERE category_id = ?1 ORDER BY tool",
            [category_id],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        if let Some(t) = row
            .get_value(0)
            .ok()
            .and_then(|v| v.as_text().cloned())
            .filter(|s| !s.is_empty())
        {
            out.push(t);
        }
    }
    drop(rows);
    Ok(out)
}

/// Replace the whole grant set for a category (empty vec clears the restriction).
pub async fn set_all(
    conn: &Connection,
    category_id: &str,
    tools: &[String],
) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM category_tools WHERE category_id = ?1",
        [category_id],
    )
    .await?;
    let now = now_rfc3339();
    for tool in tools {
        let tool = tool.trim();
        if tool.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO category_tools (category_id, tool, created_at) VALUES (?1,?2,?3)",
            (category_id, tool, now.as_str()),
        )
        .await?;
    }
    Ok(())
}
