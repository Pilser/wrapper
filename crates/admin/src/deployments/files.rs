//! Binary / data files shipped in a deployment package (rows of
//! `deployment_configs` with `config_type = "file"`).

use shared::{DeploymentConfig, Error, Result};
use telemetry::{Level, Telemetry};

/// Stage a file for a category's deployment package and persist it via
/// `db::global::deployment_configs::insert`.
pub async fn push_file(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    category_id: &str,
    name: &str,
    path: &str,
) -> Result<()> {
    if category_id.trim().is_empty() || name.trim().is_empty() || path.trim().is_empty()
    {
        return Err(Error::InvalidInput(
            "category_id, name and path must not be empty".into(),
        ));
    }
    let now = shared::now_rfc3339();
    let entry = DeploymentConfig {
        id: format!("dep_{}", uuid::Uuid::new_v4()),
        category_id: category_id.to_string(),
        config_type: "file".to_string(),
        name: name.to_string(),
        path: path.to_string(),
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
        .source("admin::deployments::files")
        .message(format!("file {name} staged for category {category_id}"))
        .data("category_id", category_id.to_string())
        .data("deployment_id", entry.id.clone())
        .tag("admin")
        .tag("deployment")
        .emit();
    Ok(())
}

/// Remove a staged file from a category's deployment package.
pub async fn remove_file(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    category_id: &str,
    name: &str,
) -> Result<()> {
    if category_id.trim().is_empty() || name.trim().is_empty() {
        return Err(Error::InvalidInput(
            "category_id and name must not be empty".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let entry = db::global::deployment_configs::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .into_iter()
        .find(|c| {
            c.category_id == category_id && c.config_type == "file" && c.name == name
        })
        .ok_or_else(|| {
            Error::NotFound(format!("file {name} for category {category_id}"))
        })?;
    db::global::deployment_configs::delete(&conn, &entry.id)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.deployment.configured")
        .category("admin")
        .source("admin::deployments::files")
        .message(format!("file {name} removed from category {category_id}"))
        .data("category_id", category_id.to_string())
        .tag("admin")
        .tag("deployment")
        .emit();
    Ok(())
}
