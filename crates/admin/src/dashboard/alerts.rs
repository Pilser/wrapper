//! Dashboard alerts (derived from live global-DB state, raised via telemetry).

use shared::{Error, Result, TenantStatus};
use telemetry::{Level, Telemetry};

/// An alert visible on the admin dashboard.
#[derive(Debug, Clone)]
pub struct Alert {
    pub level: String,
    pub message: String,
}

/// List active alerts: one per suspended tenant plus one per tenant without a
/// category assignment.
pub async fn list_alerts(db: &db::GlobalDb) -> Result<Vec<Alert>> {
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let tenants = db::global::tenants::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let mut alerts = Vec::new();
    for tenant in &tenants {
        if tenant.status == TenantStatus::Suspended {
            alerts.push(Alert {
                level: "warn".to_string(),
                message: format!("tenant {} is suspended", tenant.id),
            });
        }
        if tenant.category_id.trim().is_empty() {
            alerts.push(Alert {
                level: "warn".to_string(),
                message: format!("tenant {} has no category", tenant.id),
            });
        }
    }
    Ok(alerts)
}

/// Raise an alert via telemetry.
pub fn raise_alert(telemetry: &Telemetry, level: &str, message: &str) -> Result<()> {
    if message.trim().is_empty() {
        return Err(Error::InvalidInput(
            "alert message must not be empty".into(),
        ));
    }
    let event_level = match level.to_ascii_lowercase().as_str() {
        "error" | "critical" => Level::Error,
        "warn" | "warning" => Level::Warn,
        _ => Level::Info,
    };
    telemetry
        .builder()
        .level(event_level)
        .kind("system.health_check.failed")
        .category("admin")
        .source("admin::dashboard::alerts")
        .message(message.to_string())
        .tag("admin")
        .tag("alert")
        .emit();
    Ok(())
}
