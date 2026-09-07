//! Supervisor / agent plugins shipped in a deployment package (rows of
//! `deployment_configs` with `config_type = "plugin"`).

use shared::{DeploymentConfig, Error, Result};
use telemetry::{Level, Telemetry};

/// A plugin artifact (`.so`) deployed to the supervisor or the agent.
#[derive(Debug, Clone)]
pub struct DeploymentPlugin {
    pub name: String,
    pub path: String,
    pub supervisor_side: bool,
}

/// Record that a plugin belongs to a category's deployment package and
/// persist it via `db::global::deployment_configs::insert`.
pub async fn add_plugin(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    category_id: &str,
    plugin: DeploymentPlugin,
) -> Result<()> {
    if category_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id must not be empty".into(),
        ));
    }
    if plugin.name.trim().is_empty() {
        return Err(Error::InvalidInput(
            "plugin name must not be empty".into(),
        ));
    }
    let now = shared::now_rfc3339();
    let entry = DeploymentConfig {
        id: format!("dep_{}", uuid::Uuid::new_v4()),
        category_id: category_id.to_string(),
        config_type: "plugin".to_string(),
        name: plugin.name.clone(),
        path: plugin.path.clone(),
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
        .source("admin::deployments::plugins")
        .message(format!(
            "plugin {} added to category {category_id}",
            plugin.name
        ))
        .data("category_id", category_id.to_string())
        .data("plugin", plugin.name.clone())
        .data("deployment_id", entry.id.clone())
        .tag("admin")
        .tag("deployment")
        .emit();
    Ok(())
}

/// Record that a plugin was removed from a category's deployment package.
pub async fn remove_plugin(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    category_id: &str,
    plugin_name: &str,
) -> Result<()> {
    if category_id.trim().is_empty() || plugin_name.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id and plugin_name must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let entry = db::global::deployment_configs::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .into_iter()
        .find(|c| {
            c.category_id == category_id
                && c.config_type == "plugin"
                && c.name == plugin_name
        })
        .ok_or_else(|| {
            Error::NotFound(format!(
                "plugin {plugin_name} for category {category_id}"
            ))
        })?;
    db::global::deployment_configs::delete(&conn, &entry.id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.deployment.configured")
        .category("admin")
        .source("admin::deployments::plugins")
        .message(format!(
            "plugin {plugin_name} removed from category {category_id}"
        ))
        .data("category_id", category_id.to_string())
        .tag("admin")
        .tag("deployment")
        .emit();
    Ok(())
}
