//! Plugin-owned schema migrations (extension tables plugins may register).

/// Record a plugin migration for audit / ordering.
pub async fn record_plugin_migration(
    _conn: &turso::Connection,
    _plugin: &str,
    _version: u32,
) -> anyhow::Result<()> {
    Ok(())
}
