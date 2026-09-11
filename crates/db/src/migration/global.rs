//! Global schema migration.

const GLOBAL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL, description TEXT,
    vm_template_id TEXT NOT NULL, max_tenants INTEGER, status TEXT DEFAULT 'active',
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS vm_templates (
    id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL, description TEXT,
    rootfs_path TEXT NOT NULL, kernel_path TEXT NOT NULL, initrd_path TEXT NOT NULL,
    vcpu_count INTEGER DEFAULT 2, memory_mb INTEGER DEFAULT 2048,
    requires_gpu INTEGER DEFAULT 0, status TEXT DEFAULT 'active',
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tenants (
    id TEXT PRIMARY KEY, username TEXT UNIQUE NOT NULL, password_hash TEXT NOT NULL,
    display_name TEXT, category_id TEXT NOT NULL, vm_ip TEXT, vm_process_id INTEGER,
    vm_template_id TEXT, tap_device TEXT, status TEXT DEFAULT 'active',
    db_path TEXT NOT NULL, created_at TEXT NOT NULL, last_login TEXT,
    category_changed_at TEXT);
CREATE TABLE IF NOT EXISTS category_assignments (
    id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, category_id TEXT NOT NULL,
    assigned_by TEXT, assigned_at TEXT NOT NULL, migrated INTEGER DEFAULT 0, notes TEXT);
CREATE TABLE IF NOT EXISTS deployment_configs (
    id TEXT PRIMARY KEY, category_id TEXT NOT NULL, config_type TEXT NOT NULL,
    name TEXT NOT NULL, path TEXT NOT NULL, content BLOB, checksum TEXT,
    priority INTEGER DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS networks (
    id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, ip_address TEXT NOT NULL,
    tap_device TEXT NOT NULL, allocated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, token TEXT UNIQUE NOT NULL,
    ip_address TEXT, created_at TEXT NOT NULL, expires_at TEXT NOT NULL,
    last_active TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS staff (
    id TEXT PRIMARY KEY, username TEXT UNIQUE NOT NULL, password_hash TEXT NOT NULL,
    role TEXT NOT NULL, created_at TEXT NOT NULL);
-- Per-category MCP tool grants for in-VM supervisor tools. No rows for a
-- category = all tools allowed (backward compatible). Rows present = exactly
-- these dotted op names (e.g. memory.save) are listed and callable.
CREATE TABLE IF NOT EXISTS category_tools (
    category_id TEXT NOT NULL, tool TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (category_id, tool));
"#;

pub async fn migrate(conn: &turso::Connection) -> anyhow::Result<()> {
    conn.execute_batch(GLOBAL_SCHEMA).await?;
    Ok(())
}
