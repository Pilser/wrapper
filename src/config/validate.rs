//! Validate a parsed [`super::Config`].

use shared::{Error, Result};

use super::Config;

fn non_empty(value: &str, what: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::Config(format!("{what} must not be empty")));
    }
    Ok(())
}

/// Check ports, paths and enum-like values.
pub fn validate(cfg: &Config) -> Result<()> {
    if cfg.wrapper.port == 0 {
        return Err(Error::Config("wrapper.port must be non-zero".into()));
    }
    non_empty(&cfg.wrapper.bind_address, "wrapper.bind_address")?;
    non_empty(&cfg.firecracker.binary_path, "firecracker.binary_path")?;
    non_empty(&cfg.network.ip_range, "network.ip_range")?;
    non_empty(&cfg.network.gateway, "network.gateway")?;
    non_empty(&cfg.database.global_path, "database.global_path")?;
    non_empty(&cfg.database.tenants_dir, "database.tenants_dir")?;
    if cfg.supervisor_server.port == 0 {
        return Err(Error::Config(
            "supervisor_server.port must be non-zero".into(),
        ));
    }
    non_empty(
        &cfg.supervisor_server.bind_address,
        "supervisor_server.bind_address",
    )?;
    non_empty(&cfg.plugins.directory, "plugins.directory")?;
    non_empty(&cfg.categories.config_path, "categories.config_path")?;
    non_empty(&cfg.deployments.storage_path, "deployments.storage_path")?;
    if cfg.deployments.default_push_port == 0 {
        return Err(Error::Config(
            "deployments.default_push_port must be non-zero".into(),
        ));
    }
    match cfg.telemetry.output.as_str() {
        "disk" | "broadcast" | "both" | "none" => {}
        other => {
            return Err(Error::Config(format!(
                "telemetry.output must be disk|broadcast|both|none, got {other}"
            )));
        }
    }
    non_empty(&cfg.telemetry.disk.path, "telemetry.disk.path")?;
    if cfg.telemetry.broadcast.port == 0 {
        return Err(Error::Config(
            "telemetry.broadcast.port must be non-zero".into(),
        ));
    }
    non_empty(
        &cfg.telemetry.filters.min_level,
        "telemetry.filters.min_level",
    )?;
    Ok(())
}
