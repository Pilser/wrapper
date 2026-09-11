use shared::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `reports.*` calls (tenant `reports` table).
pub async fn handle_reports_call(
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
        .message(format!("reports tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("list");
    let result = match op {
        "create" | "insert" | "add" => {
            let period_start = super::str_arg(payload, "period_start").ok_or_else(|| {
                Error::InvalidInput("reports.create missing `period_start`".to_string())
            })?;
            let period_end = super::str_arg(payload, "period_end").ok_or_else(|| {
                Error::InvalidInput("reports.create missing `period_end`".to_string())
            })?;
            let content = super::str_arg(payload, "content").ok_or_else(|| {
                Error::InvalidInput("reports.create missing `content`".to_string())
            })?;
            let row = db::tenant::reports::ReportRow {
                id: super::str_arg(payload, "id")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                period_start: period_start.to_string(),
                period_end: period_end.to_string(),
                content: content.to_string(),
                highlights: super::str_arg(payload, "highlights").map(|s| s.to_string()),
                areas_to_improve: super::str_arg(payload, "areas_to_improve")
                    .map(|s| s.to_string()),
                generated_at: shared::now_rfc3339(),
            };
            db::tenant::reports::insert(&conn, &row)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&row).map_err(Error::from)?
        }
        "get" => {
            // No id → list (supervisor get_report schema keeps id optional).
            if super::str_arg(payload, "id").is_none() {
                let since = super::str_arg(payload, "since").or_else(|| super::str_arg(payload, "period"));
                let until = super::str_arg(payload, "until");
                let rows = db::tenant::reports::list(&conn, since, until)
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?;
                serde_json::to_value(&rows).map_err(Error::from)?
            } else {
                let id = super::str_arg(payload, "id").unwrap_or("");
            let mut rows = conn
                .query(
                    "SELECT id, period_start, period_end, content, highlights, areas_to_improve, generated_at \
                     FROM reports WHERE id = ?1",
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
                    let entry = db::tenant::reports::ReportRow {
                        id: text(0),
                        period_start: text(1),
                        period_end: text(2),
                        content: text(3),
                        highlights: Some(text(4)).filter(|s| !s.is_empty()),
                        areas_to_improve: Some(text(5)).filter(|s| !s.is_empty()),
                        generated_at: text(6),
                    };
                    serde_json::to_value(&entry).map_err(Error::from)?
                }
                None => Value::Null,
            }
            }
        }
        _ => {
            let since = super::str_arg(payload, "since");
            let until = super::str_arg(payload, "until");
            let rows = db::tenant::reports::list(&conn, since, until)
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
        .message(format!("reports tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
