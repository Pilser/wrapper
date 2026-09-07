//! Post-deployment commands (setup / initialization scripts) persisted as
//! rows of `deployment_configs` with `config_type = "command"`.

use shared::{DeploymentConfig, Error, Result};
use telemetry::{Level, Telemetry};

/// Queue a script to run inside the VM after a deployment lands and persist
/// it via `db::global::deployment_configs::insert`.
pub async fn queue_command(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    category_id: &str,
    script: &str,
) -> Result<()> {
    if category_id.trim().is_empty() || script.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id and script must not be empty".into(),
        ));
    }
    let now = shared::now_rfc3339();
    let entry = DeploymentConfig {
        id: format!("dep_{}", uuid::Uuid::new_v4()),
        category_id: category_id.to_string(),
        config_type: "command".to_string(),
        name: script.to_string(),
        path: script.to_string(),
        content: None,
        checksum: None,
        priority: 0,
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
        .source("admin::deployments::commands")
        .message(format!(
            "command {script} queued for category {category_id}"
        ))
        .data("category_id", category_id.to_string())
        .data("deployment_id", entry.id.clone())
        .tag("admin")
        .tag("deployment")
        .emit();
    Ok(())
}
