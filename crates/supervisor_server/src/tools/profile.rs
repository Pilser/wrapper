use shared::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `profile.*` calls (tenant `profiles` row).
pub async fn handle_profile_call(
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
        .message(format!("profile tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("get");
    let result = match op {
        "update" | "set" | "upsert" => {
            let now = shared::now_rfc3339();
            let existing = db::tenant::profile::get(&conn, tenant_id)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            let agent_name = super::str_arg(payload, "agent_name")
                .map(|s| s.to_string())
                .or_else(|| existing.as_ref().map(|p| p.agent_name.clone()))
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    Error::InvalidInput("profile.update missing `agent_name`".to_string())
                })?;
            let profile = db::tenant::profile::Profile {
                tenant_id: tenant_id.to_string(),
                agent_name,
                nickname: super::str_arg(payload, "nickname")
                    .map(|s| s.to_string())
                    .or_else(|| existing.as_ref().and_then(|p| p.nickname.clone())),
                language: super::str_arg(payload, "language")
                    .map(|s| s.to_string())
                    .or_else(|| existing.as_ref().map(|p| p.language.clone()))
                    .unwrap_or_else(|| "en".to_string()),
                timezone: super::str_arg(payload, "timezone")
                    .map(|s| s.to_string())
                    .or_else(|| existing.as_ref().map(|p| p.timezone.clone()))
                    .unwrap_or_else(|| "UTC".to_string()),
                created_at: existing
                    .as_ref()
                    .map(|p| p.created_at.clone())
                    .unwrap_or_else(|| now.clone()),
                updated_at: now,
            };
            db::tenant::profile::upsert(&conn, &profile)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            serde_json::to_value(&profile).map_err(Error::from)?
        }
        _ => {
            match db::tenant::profile::get(&conn, tenant_id)
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
        .message(format!("profile tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
