use shared::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `skills.*` calls (tenant `skills` table).
pub async fn handle_skills_call(
    tenant_id: &str,
    payload: &Value,
    db: &db::Db,
    telemetry: &Telemetry,
) -> Result<Value> {
    crate::validate_tenant_id(tenant_id)?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.tool.called")
        .category("supervisor")
        .message(format!("skills tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("list");
    let result = match op {
        "update" => {
            // Read-modify-write: only provided fields change.
            let id = super::str_arg(payload, "id").ok_or_else(|| {
                Error::InvalidInput("skills.update missing `id`".to_string())
            })?;
            let mut rows = conn
                .query("SELECT id, name, domain, level, confidence, last_practiced, created_at FROM skills WHERE id = ?1", [id])
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            let cur = rows
                .next()
                .await
                .map_err(|e| Error::Database(e.to_string()))?
                .ok_or_else(|| Error::NotFound(format!("skill {id}")))?;
            drop(rows);
            let text = |i: usize| {
                cur.get_value(i)
                    .ok()
                    .and_then(|v| v.as_text().cloned())
                    .unwrap_or_default()
            };
            let num = |i: usize| {
                cur.get_value(i)
                    .ok()
                    .and_then(|v| {
                        v.as_real()
                            .copied()
                            .or_else(|| v.as_integer().copied().map(|n| n as f64))
                    })
                    .unwrap_or(0.0)
            };
            let upd = |key: &str, cur_v: String| {
                super::str_arg(payload, key)
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(cur_v)
            };
            conn.execute(
                "UPDATE skills SET name = ?1, domain = ?2, level = ?3, confidence = ?4, last_practiced = ?5 WHERE id = ?6",
                (
                    upd("name", text(1)).as_str(),
                    upd("domain", text(2)).as_str(),
                    upd("level", text(3)).as_str(),
                    super::float_arg(payload, "confidence").unwrap_or_else(|| num(4)),
                    upd("last_practiced", text(5)).as_str(),
                    id,
                ),
            )
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"ok": true, "updated": id})
        }
        "create" | "insert" | "add" => {
            let name = super::str_arg(payload, "name").ok_or_else(|| {
                Error::InvalidInput("skills.create missing `name`".to_string())
            })?;
            let row = db::tenant::skills::SkillRow {
                id: super::str_arg(payload, "id")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: name.to_string(),
                domain: super::str_arg(payload, "domain").map(|s| s.to_string()),
                level: super::str_arg(payload, "level")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "beginner".to_string()),
                confidence: super::float_arg(payload, "confidence").unwrap_or(0.5),
                last_practiced: super::str_arg(payload, "last_practiced")
                    .map(|s| s.to_string()),
                created_at: shared::now_rfc3339(),
            };
            db::tenant::skills::insert(&conn, &row)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "get" => {
            let id = super::str_arg(payload, "id").ok_or_else(|| {
                Error::InvalidInput("skills.get missing `id`".to_string())
            })?;
            let mut rows = conn
                .query(
                    "SELECT id, name, domain, level, confidence, last_practiced, created_at \
                     FROM skills WHERE id = ?1",
                    [id],
                )
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            match rows
                .next()
                .await
                .map_err(|e| Error::Database(e.to_string()))?
            {
                Some(row) => {
                    let text = |i: usize| {
                        row.get_value(i)
                            .ok()
                            .and_then(|v| v.as_text().cloned())
                            .unwrap_or_default()
                    };
                    let real = |i: usize| {
                        row.get_value(i)
                            .ok()
                            .and_then(|v| v.as_real().copied())
                            .unwrap_or(0.5)
                    };
                    let entry = db::tenant::skills::SkillRow {
                        id: text(0),
                        name: text(1),
                        domain: Some(text(2)).filter(|s| !s.is_empty()),
                        level: text(3),
                        confidence: real(4),
                        last_practiced: Some(text(5)).filter(|s| !s.is_empty()),
                        created_at: text(6),
                    };
                    serde_json::to_value(&entry).map_err(Error::from)?
                }
                None => Value::Null,
            }
        }
        _ => {
            let domain = super::str_arg(payload, "domain");
            let rows = db::tenant::skills::list(&conn, domain)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
    };
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.tool.completed")
        .category("supervisor")
        .message(format!("skills tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
