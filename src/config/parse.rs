//! Minimal TOML parser for the known `config.toml` shape.
//!
//! The workspace has no `toml` dependency, so this parses the subset we
//! write: `[section]` / `[section.subsection]` headers, `key = value` pairs
//! with quoted strings, booleans, integers and single-line string/integer
//! arrays. Missing keys fall back to [`super::Config`] defaults.

use std::collections::HashMap;

use shared::{Error, Result};

use super::Config;

type Table = HashMap<String, HashMap<String, String>>;

/// Parse TOML text into a [`Config`].
pub fn parse(text: &str) -> Result<Config> {
    let mut table: Table = HashMap::new();
    let mut current = String::new();
    for (index, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim().to_string();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].trim().to_string();
            if current.is_empty() {
                return Err(Error::Config(format!(
                    "line {}: empty section header",
                    index + 1
                )));
            }
            table.entry(current.clone()).or_default();
        } else if let Some((key, value)) = line.split_once('=') {
            if current.is_empty() {
                return Err(Error::Config(format!(
                    "line {}: key outside of any section",
                    index + 1
                )));
            }
            table
                .entry(current.clone())
                .or_default()
                .insert(key.trim().to_string(), value.trim().to_string());
        } else {
            return Err(Error::Config(format!(
                "line {}: cannot parse: {raw}",
                index + 1
            )));
        }
    }

    let mut cfg = Config::default();
    cfg.wrapper.bind_address = str_val(&table, "wrapper", "bind_address", cfg.wrapper.bind_address)?;
    cfg.wrapper.port = u16_val(&table, "wrapper", "port", cfg.wrapper.port)?;
    cfg.firecracker.binary_path = str_val(
        &table,
        "firecracker",
        "binary_path",
        cfg.firecracker.binary_path,
    )?;
    cfg.firecracker.kernel_path = str_val(
        &table,
        "firecracker",
        "kernel_path",
        cfg.firecracker.kernel_path,
    )?;
    cfg.firecracker.initrd_path = str_val(
        &table,
        "firecracker",
        "initrd_path",
        cfg.firecracker.initrd_path,
    )?;
    cfg.network.ip_range = str_val(&table, "network", "ip_range", cfg.network.ip_range)?;
    cfg.network.gateway = str_val(&table, "network", "gateway", cfg.network.gateway)?;
    cfg.database.global_path = str_val(
        &table,
        "database",
        "global_path",
        cfg.database.global_path,
    )?;
    cfg.database.tenants_dir = str_val(
        &table,
        "database",
        "tenants_dir",
        cfg.database.tenants_dir,
    )?;
    cfg.supervisor_server.bind_address = str_val(
        &table,
        "supervisor_server",
        "bind_address",
        cfg.supervisor_server.bind_address,
    )?;
    cfg.supervisor_server.port = u16_val(&table, "supervisor_server", "port", cfg.supervisor_server.port)?;
    cfg.supervisor_server.heartbeat_timeout_seconds = u64_val(
        &table,
        "supervisor_server",
        "heartbeat_timeout_seconds",
        cfg.supervisor_server.heartbeat_timeout_seconds,
    )?;
    cfg.plugins.directory =
        str_val(&table, "plugins", "directory", cfg.plugins.directory)?;
    cfg.plugins.hot_reload = bool_val(&table, "plugins", "hot_reload", cfg.plugins.hot_reload)?;
    cfg.categories.config_path = str_val(
        &table,
        "categories",
        "config_path",
        cfg.categories.config_path,
    )?;
    cfg.deployments.storage_path = str_val(
        &table,
        "deployments",
        "storage_path",
        cfg.deployments.storage_path,
    )?;
    cfg.deployments.default_push_port = u16_val(
        &table,
        "deployments",
        "default_push_port",
        cfg.deployments.default_push_port,
    )?;
    cfg.vm.idle_timeout_minutes = u64_val(
        &table,
        "vm",
        "idle_timeout_minutes",
        cfg.vm.idle_timeout_minutes,
    )?;
    cfg.api.public_base = str_val(&table, "api", "public_base", cfg.api.public_base)?;
    cfg.telemetry.enabled = bool_val(&table, "telemetry", "enabled", cfg.telemetry.enabled)?;
    cfg.telemetry.output = str_val(&table, "telemetry", "output", cfg.telemetry.output)?;
    cfg.telemetry.disk.path = str_val(
        &table,
        "telemetry.disk",
        "path",
        cfg.telemetry.disk.path,
    )?;
    cfg.telemetry.disk.filename_pattern = str_val(
        &table,
        "telemetry.disk",
        "filename_pattern",
        cfg.telemetry.disk.filename_pattern,
    )?;
    cfg.telemetry.disk.buffer_size = usize_val(
        &table,
        "telemetry.disk",
        "buffer_size",
        cfg.telemetry.disk.buffer_size,
    )?;
    cfg.telemetry.disk.flush_interval_seconds = u64_val(
        &table,
        "telemetry.disk",
        "flush_interval_seconds",
        cfg.telemetry.disk.flush_interval_seconds,
    )?;
    cfg.telemetry.disk.compress_old_files = bool_val(
        &table,
        "telemetry.disk",
        "compress_old_files",
        cfg.telemetry.disk.compress_old_files,
    )?;
    cfg.telemetry.disk.retention_days = u32_val(
        &table,
        "telemetry.disk",
        "retention_days",
        cfg.telemetry.disk.retention_days,
    )?;
    cfg.telemetry.broadcast.bind_address = str_val(
        &table,
        "telemetry.broadcast",
        "bind_address",
        cfg.telemetry.broadcast.bind_address,
    )?;
    cfg.telemetry.broadcast.port = u16_val(
        &table,
        "telemetry.broadcast",
        "port",
        cfg.telemetry.broadcast.port,
    )?;
    cfg.telemetry.broadcast.protocol = str_val(
        &table,
        "telemetry.broadcast",
        "protocol",
        cfg.telemetry.broadcast.protocol,
    )?;
    cfg.telemetry.broadcast.max_clients = usize_val(
        &table,
        "telemetry.broadcast",
        "max_clients",
        cfg.telemetry.broadcast.max_clients,
    )?;
    cfg.telemetry.broadcast.events_per_second_per_client = u32_val(
        &table,
        "telemetry.broadcast",
        "events_per_second_per_client",
        cfg.telemetry.broadcast.events_per_second_per_client,
    )?;
    cfg.telemetry.broadcast.auth_required = bool_val(
        &table,
        "telemetry.broadcast",
        "auth_required",
        cfg.telemetry.broadcast.auth_required,
    )?;
    cfg.telemetry.metrics.enabled = bool_val(
        &table,
        "telemetry.metrics",
        "enabled",
        cfg.telemetry.metrics.enabled,
    )?;
    cfg.telemetry.metrics.collection_interval_seconds = u64_val(
        &table,
        "telemetry.metrics",
        "collection_interval_seconds",
        cfg.telemetry.metrics.collection_interval_seconds,
    )?;
    cfg.telemetry.metrics.track_latency_percentiles = u32_list_val(
        &table,
        "telemetry.metrics",
        "track_latency_percentiles",
        cfg.telemetry.metrics.track_latency_percentiles,
    )?;
    cfg.telemetry.filters.exclude_from_disk = string_list_val(
        &table,
        "telemetry.filters",
        "exclude_from_disk",
        cfg.telemetry.filters.exclude_from_disk,
    )?;
    cfg.telemetry.filters.min_level = str_val(
        &table,
        "telemetry.filters",
        "min_level",
        cfg.telemetry.filters.min_level,
    )?;
    Ok(cfg)
}

/// Cut a trailing `# comment`, ignoring `#` inside quotes.
fn strip_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    for (i, c) in line.char_indices() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return &line[..i],
            _ => {}
        }
    }
    line
}

fn raw<'a>(table: &'a Table, section: &str, key: &str) -> Option<&'a str> {
    table.get(section)?.get(key).map(String::as_str)
}

fn unquote(value: &str, section: &str, key: &str) -> Result<String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Ok(value[1..value.len() - 1].to_string())
    } else {
        Err(Error::Config(format!(
            "[{section}] {key}: expected quoted string, got {value}"
        )))
    }
}

fn str_val(table: &Table, section: &str, key: &str, fallback: String) -> Result<String> {
    match raw(table, section, key) {
        Some(v) => unquote(v, section, key),
        None => Ok(fallback),
    }
}

fn bool_val(table: &Table, section: &str, key: &str, fallback: bool) -> Result<bool> {
    match raw(table, section, key) {
        Some(v) => match v.trim() {
            "true" => Ok(true),
            "false" => Ok(false),
            other => Err(Error::Config(format!(
                "[{section}] {key}: expected bool, got {other}"
            ))),
        },
        None => Ok(fallback),
    }
}

fn int_val<T>(table: &Table, section: &str, key: &str, fallback: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match raw(table, section, key) {
        Some(v) => v.trim().parse::<T>().map_err(|e| {
            Error::Config(format!("[{section}] {key}: expected integer: {e}"))
        }),
        None => Ok(fallback),
    }
}

fn u16_val(table: &Table, section: &str, key: &str, fallback: u16) -> Result<u16> {
    int_val(table, section, key, fallback)
}

fn u32_val(table: &Table, section: &str, key: &str, fallback: u32) -> Result<u32> {
    int_val(table, section, key, fallback)
}

fn u64_val(table: &Table, section: &str, key: &str, fallback: u64) -> Result<u64> {
    int_val(table, section, key, fallback)
}

fn usize_val(table: &Table, section: &str, key: &str, fallback: usize) -> Result<usize> {
    int_val(table, section, key, fallback)
}

fn array_items(value: &str, section: &str, key: &str) -> Result<Vec<String>> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(Error::Config(format!(
            "[{section}] {key}: expected array, got {value}"
        )));
    }
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    Ok(inner
        .split(',')
        .map(|item| item.trim().to_string())
        .collect())
}

fn u32_list_val(
    table: &Table,
    section: &str,
    key: &str,
    fallback: Vec<u32>,
) -> Result<Vec<u32>> {
    match raw(table, section, key) {
        Some(v) => array_items(v, section, key)?
            .into_iter()
            .map(|item| {
                item.parse::<u32>().map_err(|e| {
                    Error::Config(format!("[{section}] {key}: expected integer: {e}"))
                })
            })
            .collect(),
        None => Ok(fallback),
    }
}

fn string_list_val(
    table: &Table,
    section: &str,
    key: &str,
    fallback: Vec<String>,
) -> Result<Vec<String>> {
    match raw(table, section, key) {
        Some(v) => array_items(v, section, key)?
            .into_iter()
            .map(|item| unquote(&item, section, key))
            .collect(),
        None => Ok(fallback),
    }
}
