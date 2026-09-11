use shared::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `memory.*` calls against the tenant `memory` table.
pub async fn handle_memory_call(
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
        .message(format!("memory tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("list");
    let result = match op {
        "save" => {
            let key = super::str_arg(payload, "key").unwrap_or("").trim();
            if key.is_empty() {
                return Err(Error::InvalidInput("memory.save missing `key`".to_string()));
            }
            let now = shared::now_rfc3339();
            let row = db::tenant::memory::MemoryRow {
                id: uuid::Uuid::new_v4().to_string(),
                key: key.to_string(),
                value: super::str_arg(payload, "value").unwrap_or("").to_string(),
                category: super::str_arg(payload, "category").unwrap_or("").to_string(),
                importance: super::float_arg(payload, "importance").unwrap_or(0.0),
                created_at: now.clone(),
                updated_at: now,
            };
            db::tenant::memory::insert(&conn, &row)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "search" => {
            let q = super::str_arg(payload, "query")
                .or_else(|| super::str_arg(payload, "q"))
                .unwrap_or("");
            let rows = db::tenant::memory::search(&conn, q)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&rows).map_err(Error::from)?
        }
        "delete" => {
            let key = super::str_arg(payload, "key").unwrap_or("").trim();
            if key.is_empty() {
                return Err(Error::InvalidInput("memory.delete missing `key`".to_string()));
            }
            db::tenant::memory::delete(&conn, key)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::json!({"ok": true, "deleted": key})
        }
        "get" => {
            let key = payload.get("key").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() {
                return Err(Error::InvalidInput("memory.get missing `key`".to_string()));
            }
            match db::tenant::memory::get_by_key(&conn, key)
                .await
                .map_err(|e| Error::Database(e.to_string()))?
            {
                Some(row) => serde_json::to_value(&row).map_err(Error::from)?,
                None => Value::Null,
            }
        }
        _ => {
            let category = payload.get("category").and_then(|v| v.as_str());
            let rows = db::tenant::memory::list(&conn, category, Some(100))
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
        .message(format!("memory tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
