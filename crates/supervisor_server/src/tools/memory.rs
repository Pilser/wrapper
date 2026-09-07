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
