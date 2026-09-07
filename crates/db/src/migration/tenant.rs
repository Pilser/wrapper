//! Per-tenant schema migration.

const TENANT_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS profiles (
    tenant_id TEXT PRIMARY KEY, agent_name TEXT NOT NULL, nickname TEXT,
    language TEXT DEFAULT 'en', timezone TEXT DEFAULT 'UTC',
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS memory (
    id TEXT PRIMARY KEY, key TEXT NOT NULL, value TEXT NOT NULL,
    category TEXT NOT NULL, importance REAL DEFAULT 0.5,
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS history (
    id TEXT PRIMARY KEY, session_id TEXT, action TEXT NOT NULL,
    domain TEXT, summary TEXT, duration_seconds INTEGER, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, domain TEXT, status TEXT DEFAULT 'active',
    description TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS skills (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, domain TEXT, level TEXT DEFAULT 'beginner',
    confidence REAL DEFAULT 0.5, last_practiced TEXT, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS reports (
    id TEXT PRIMARY KEY, period_start TEXT NOT NULL, period_end TEXT NOT NULL,
    content TEXT NOT NULL, highlights TEXT, areas_to_improve TEXT,
    generated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS configs (
    id TEXT PRIMARY KEY, key TEXT UNIQUE NOT NULL, value TEXT NOT NULL,
    updated_at TEXT NOT NULL);
"#;

pub async fn migrate(conn: &turso::Connection) -> anyhow::Result<()> {
    conn.execute_batch(TENANT_SCHEMA).await?;
    Ok(())
}
