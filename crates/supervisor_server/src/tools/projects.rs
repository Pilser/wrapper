use core::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `projects.*` calls (tenant `projects` table).
pub async fn handle_projects_call(
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
        .message(format!("projects tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("list");
    let result = match op {
        "create" | "insert" | "add" => {
            let name = super::str_arg(payload, "name").ok_or_else(|| {
                Error::InvalidInput("projects.create missing `name`".to_string())
            })?;
            let now = core::now_rfc3339();
            let row = db::tenant::projects::ProjectRow {
                id: super::str_arg(payload, "id")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: name.to_string(),
                domain: super::str_arg(payload, "domain").map(|s| s.to_string()),
                status: super::str_arg(payload, "status")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "active".to_string()),
                description: super::str_arg(payload, "description").map(|s| s.to_string()),
                created_at: now.clone(),
                updated_at: now,
            };
            db::tenant::projects::insert(&conn, &row)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "get" => {
            let id = super::str_arg(payload, "id").ok_or_else(|| {
                Error::InvalidInput("projects.get missing `id`".to_string())
            })?;
            let mut rows = conn
                .query(
                    "SELECT id, name, domain, status, description, created_at, updated_at \
                     FROM projects WHERE id = ?1",
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
                    let entry = db::tenant::projects::ProjectRow {
                        id: text(0),
                        name: text(1),
                        domain: Some(text(2)).filter(|s| !s.is_empty()),
                        status: text(3),
                        description: Some(text(4)).filter(|s| !s.is_empty()),
                        created_at: text(5),
                        updated_at: text(6),
                    };
                    serde_json::to_value(&entry).map_err(Error::from)?
                }
                None => Value::Null,
            }
        }
        _ => {
            let status = super::str_arg(payload, "status");
            let rows = db::tenant::projects::list(&conn, status)
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
        .message(format!("projects tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
