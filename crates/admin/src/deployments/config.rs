//! Configure a deployment entry (one row of `deployment_configs`).

use core::{now_rfc3339, DeploymentConfig, Error, Result};
use telemetry::{Level, Telemetry};

/// Input for [`configure_deployment`]; mirrors one row of the
/// `deployment_configs` table.
#[derive(Debug, Clone)]
pub struct ConfigureDeployment {
    pub category_id: String,
    pub config_type: String,
    pub name: String,
    pub path: String,
    pub priority: i32,
}

/// Validate, persist via `db::global::deployment_configs::insert`, and emit
/// `admin.deployment.configured`.
pub async fn configure_deployment(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    input: ConfigureDeployment,
) -> Result<DeploymentConfig> {
    if input.category_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id must not be empty".into(),
        ));
    }
    if input.name.trim().is_empty() {
        return Err(Error::InvalidInput("name must not be empty".into()));
    }
    if input.path.trim().is_empty() {
        return Err(Error::InvalidInput("path must not be empty".into()));
    }
    let now = now_rfc3339();
    let entry = DeploymentConfig {
        id: format!("dep_{}", uuid::Uuid::new_v4()),
        category_id: input.category_id,
        config_type: input.config_type,
        name: input.name,
        path: input.path,
        content: None,
        checksum: None,
        priority: input.priority,
        created_at: now.clone(),
        updated_at: now,
    };
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::deployment_configs::insert(&conn, &entry)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.deployment.configured")
        .category("admin")
        .source("admin::deployments::config")
        .message(format!("deployment {} configured", entry.id))
        .data("deployment_id", entry.id.clone())
        .data("category_id", entry.category_id.clone())
        .tag("admin")
        .tag("deployment")
        .emit();
    Ok(entry)
}
