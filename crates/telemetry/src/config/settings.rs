//! Strongly-typed telemetry configuration, mirroring `[telemetry.*]` config.

use serde::{Deserialize, Serialize};

/// Top-level telemetry settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySettings {
    pub enabled: bool,
    /// `"disk"`, `"both"`, `"broadcast"` or `"none"`.
    pub output: String,
    pub disk: DiskSettings,
    pub broadcast: BroadcastSettings,
    pub metrics: MetricsSettings,
    pub filters: FilterSettings,
}

impl Default for TelemetrySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            output: "both".to_string(),
            disk: DiskSettings::default(),
            broadcast: BroadcastSettings::default(),
            metrics: MetricsSettings::default(),
            filters: FilterSettings::default(),
        }
    }
}

/// Disk persistence settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSettings {
    pub enabled: bool,
    pub path: String,
    pub filename_pattern: String,
    pub buffer_size: usize,
    pub flush_interval_seconds: u64,
    pub compress_old_files: bool,
    pub retention_days: u32,
}

impl Default for DiskSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/var/log/wrapper/telemetry".to_string(),
            filename_pattern: "telemetry_{date}.json".to_string(),
            buffer_size: 1000,
            flush_interval_seconds: 5,
            compress_old_files: true,
            retention_days: 30,
        }
    }
}

/// Live broadcast (WebSocket/SSE) settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastSettings {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    pub protocol: String,
    pub max_clients: usize,
    pub events_per_second_per_client: u32,
    pub auth_required: bool,
}

impl Default for BroadcastSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: "0.0.0.0".to_string(),
            port: 8081,
            protocol: "websocket".to_string(),
            max_clients: 10,
            events_per_second_per_client: 100,
            auth_required: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSettings {
    pub enabled: bool,
    pub collection_interval_seconds: u64,
    pub track_latency_percentiles: Vec<u32>,
}

impl Default for MetricsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_seconds: 10,
            track_latency_percentiles: vec![50, 95, 99],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterSettings {
    pub exclude_from_disk: Vec<String>,
    pub min_level: String,
}

impl Default for FilterSettings {
    fn default() -> Self {
        Self {
            exclude_from_disk: vec!["metric.gauge".to_string(), "metric.counter".to_string()],
            min_level: "info".to_string(),
        }
    }
}