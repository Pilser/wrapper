use core::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `configs.*` calls (tenant `configs` table).
pub async fn handle_configs_call(
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
        .message(format!("configs tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("get");
    let result = match op {
        "set" | "update" | "upsert" => {
            let key = super::str_arg(payload, "key").ok_or_else(|| {
                Error::InvalidInput("configs.set missing `key`".to_string())
            })?;
            let value = super::str_arg(payload, "value").ok_or_else(|| {
                Error::InvalidInput("configs.set missing `value`".to_string())
            })?;
            let now = core::now_rfc3339();
            db::tenant::configs::set(&conn, key, value, &now)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            match db::tenant::configs::get(&conn, key)
                .await
                .map_err(|e| Error::Database(e.to_string()))?
            {
                Some(row) => serde_json::to_value(&row).map_err(Error::from)?,
                None => serde_json::json!({"key": key, "value": value, "updated_at": now}),
            }
        }
        "list" => {
            let mut rows = conn
                .query(
                    "SELECT id, key, value, updated_at FROM configs ORDER BY key",
                    (),
                )
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            let mut out = Vec::new();
            while let Some(row) = rows
                .next()
                .await
                .map_err(|e| Error::Database(e.to_string()))?
            {
                let text = |i: usize| {
                    row.get_value(i)
                        .ok()
                        .and_then(|v| v.as_text().cloned())
                        .unwrap_or_default()
                };
                out.push(db::tenant::configs::ConfigRow {
                    id: text(0),
                    key: text(1),
                    value: text(2),
                    updated_at: text(3),
                });
            }
            serde_json::to_value(&out).map_err(Error::from)?
        }
        _ => {
            let key = super::str_arg(payload, "key").ok_or_else(|| {
                Error::InvalidInput("configs.get missing `key`".to_string())
            })?;
            match db::tenant::configs::get(&conn, key)
                .await
                .map_err(|e| Error::Database(e.to_string()))?
            {
                Some(row) => serde_json::to_value(&row).map_err(Error::from)?,
                None => Value::Null,
            }
        }
    };
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.tool.completed")
        .category("supervisor")
        .message(format!("configs tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
