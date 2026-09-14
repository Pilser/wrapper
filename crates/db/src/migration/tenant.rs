//! Per-tenant schema migration — portfolio ledger (hard cut, no backward compat).

const TENANT_SCHEMA: &str = r#"
DROP TABLE IF EXISTS memory;
DROP TABLE IF EXISTS history;
DROP TABLE IF EXISTS projects;
CREATE TABLE IF NOT EXISTS profiles (
    tenant_id TEXT PRIMARY KEY, agent_name TEXT NOT NULL, nickname TEXT,
    language TEXT DEFAULT 'en', timezone TEXT DEFAULT 'UTC', track TEXT DEFAULT '',
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS goals (
    id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, track TEXT DEFAULT '',
    target_date TEXT, status TEXT DEFAULT 'active', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS milestones (
    id INTEGER PRIMARY KEY AUTOINCREMENT, goal_id INTEGER NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    title TEXT NOT NULL, order_index INTEGER DEFAULT 0, status TEXT DEFAULT 'open',
    created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT, milestone_id INTEGER NOT NULL REFERENCES milestones(id) ON DELETE CASCADE,
    title TEXT NOT NULL, kind TEXT DEFAULT 'task', description TEXT DEFAULT '', difficulty TEXT DEFAULT 'medium',
    status TEXT DEFAULT 'open', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    result TEXT NOT NULL, student_output TEXT DEFAULT '', feedback TEXT DEFAULT '',
    example_solution TEXT DEFAULT '', resolved_at TEXT, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS skills (
    id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, domain TEXT DEFAULT '', level TEXT DEFAULT 'novice',
    evidence INTEGER REFERENCES attempts(id) ON DELETE SET NULL, confidence REAL DEFAULT 0.5,
    last_practiced TEXT, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS achievements (
    id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, skill_id INTEGER REFERENCES skills(id) ON DELETE SET NULL,
    task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL, awarded_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS reports (
    id INTEGER PRIMARY KEY AUTOINCREMENT, period_start TEXT NOT NULL, period_end TEXT NOT NULL,
    content TEXT NOT NULL, highlights TEXT DEFAULT '', gaps TEXT DEFAULT '', next_steps TEXT DEFAULT '',
    generated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS timeline (
    id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, ref_id INTEGER, summary TEXT NOT NULL,
    ref_type TEXT DEFAULT '', created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT, key TEXT UNIQUE NOT NULL, value TEXT NOT NULL,
    updated_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS idx_milestones_goal_id ON milestones(goal_id);
CREATE INDEX IF NOT EXISTS idx_tasks_milestone_id ON tasks(milestone_id);
CREATE INDEX IF NOT EXISTS idx_attempts_task_id ON attempts(task_id);
CREATE INDEX IF NOT EXISTS idx_timeline_created_at ON timeline(created_at);
CREATE INDEX IF NOT EXISTS idx_timeline_kind ON timeline(kind);
"#;

pub async fn migrate(conn: &turso::Connection) -> anyhow::Result<()> {
    conn.execute_batch(TENANT_SCHEMA).await?;
    // Upgrade heel for pre-portfolio DBs: profiles lacks `track`, and
    // skills/reports/configs may still carry the legacy TEXT-id shapes
    // (CREATE IF NOT EXISTS never replaces them). Best-effort, idempotent.
    let _ = conn
        .execute("ALTER TABLE profiles ADD COLUMN track TEXT DEFAULT ''", ())
        .await;
    for table in ["skills", "reports", "configs"] {
        let mut rows = match conn
            .query(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
            )
            .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };
        let mut legacy = false;
        while let Ok(Some(row)) = rows.next().await {
            let sql = row
                .get_value(0)
                .ok()
                .and_then(|v| v.as_text().cloned())
                .unwrap_or_default();
            if sql.contains("id TEXT") || sql.contains("areas_to_improve") {
                legacy = true;
            }
        }
        if legacy {
            let _ = conn
                .execute(&format!("DROP TABLE IF EXISTS {table}"), ())
                .await;
        }
    }
    // Re-run creates for anything just dropped (idempotent).
    conn.execute_batch(TENANT_SCHEMA).await?;
    Ok(())
}
