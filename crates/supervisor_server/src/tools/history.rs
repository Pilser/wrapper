use core::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `history.*` calls (tenant `history` table).
pub async fn handle_history_call(
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
        .message(format!("history tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("list");
    let result = match op {
        "append" | "insert" | "add" => {
            let action = super::str_arg(payload, "action").ok_or_else(|| {
                Error::InvalidInput("history.append missing `action`".to_string())
            })?;
            let row = db::tenant::history::HistoryRow {
                id: super::str_arg(payload, "id")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                session_id: super::str_arg(payload, "session_id").map(|s| s.to_string()),
                action: action.to_string(),
                domain: super::str_arg(payload, "domain").map(|s| s.to_string()),
                summary: super::str_arg(payload, "summary").map(|s| s.to_string()),
                duration_seconds: super::int_arg(payload, "duration_seconds"),
                created_at: core::now_rfc3339(),
            };
            db::tenant::history::insert(&conn, &row)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "get" => {
            let id = super::str_arg(payload, "id").ok_or_else(|| {
                Error::InvalidInput("history.get missing `id`".to_string())
            })?;
            let mut rows = conn
                .query(
                    "SELECT id, session_id, action, domain, summary, duration_seconds, created_at \
                     FROM history WHERE id = ?1",
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
                    let entry = db::tenant::history::HistoryRow {
                        id: text(0),
                        session_id: Some(text(1)).filter(|s| !s.is_empty()),
                        action: text(2),
                        domain: Some(text(3)).filter(|s| !s.is_empty()),
                        summary: Some(text(4)).filter(|s| !s.is_empty()),
                        duration_seconds: row
                            .get_value(5)
                            .ok()
                            .and_then(|v| v.as_integer().copied()),
                        created_at: text(6),
                    };
                    serde_json::to_value(&entry).map_err(Error::from)?
                }
                None => Value::Null,
            }
        }
        _ => {
            let domain = super::str_arg(payload, "domain");
            let limit = super::int_arg(payload, "limit").or(Some(100));
            let rows = db::tenant::history::list(&conn, domain, limit)
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
        .message(format!("history tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
