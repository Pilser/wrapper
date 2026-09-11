use shared::{Error, Result};
use serde_json::Value;
use telemetry::{Level, Telemetry};

/// Handle `tenant.info` calls (global `tenants` row + tenant DB handle).
pub async fn handle_tenant_call(
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
        .message(format!("tenant tool called tenant {tenant_id}"))
        .emit();
    let tenant_db = db.tenant(tenant_id).await?;
    let _conn = tenant_db
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let global = db.global();
    let gconn = global
        .conn()
        .map_err(|e| Error::Database(e.to_string()))?;
    let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("info");
    let result = match op {
        "category" => {
            let tenant = db::global::tenants::get_by_id(&gconn, tenant_id)
                .await
                .map_err(|e| Error::Database(e.to_string()))?
                .ok_or_else(|| Error::NotFound(format!("tenant {tenant_id}")))?;
            match db::global::categories::get_by_id(&gconn, &tenant.category_id)
                .await
                .map_err(|e| Error::Database(e.to_string()))?
            {
                Some(cat) => serde_json::to_value(&cat).map_err(Error::from)?,
                None => {
                    return Err(Error::NotFound(format!(
                        "category {}",
                        tenant.category_id
                    )));
                }
            }
        }
        _ => match db::global::tenants::get_by_id(&gconn, tenant_id)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
        {
            Some(row) => serde_json::to_value(&row).map_err(Error::from)?,
            None => {
                return Err(Error::NotFound(format!("tenant {tenant_id}")));
            }
        },
    };
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.tool.completed")
        .category("supervisor")
        .message(format!("tenant tool completed tenant {tenant_id}"))
        .emit();
    Ok(result)
}
