//! Wrapper configuration: typed `config.toml` plus load / parse / validate.

pub mod load;
pub mod parse;
pub mod validate;

pub use load::load;

use serde::{Deserialize, Serialize};

/// Full wrapper configuration, mirroring `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub wrapper: WrapperSection,
    #[serde(default)]
    pub firecracker: FirecrackerSection,
    #[serde(default)]
    pub network: NetworkSection,
    #[serde(default)]
    pub database: DatabaseSection,
    #[serde(default)]
    pub supervisor_server: SupervisorSection,
    #[serde(default)]
    pub plugins: PluginsSection,
    #[serde(default)]
    pub categories: CategoriesSection,
    #[serde(default)]
    pub deployments: DeploymentsSection,
    #[serde(default)]
    pub binaries: BinariesSection,
    #[serde(default)]
    pub vm: VmSection,
    #[serde(default)]
    pub api: ApiSection,
    #[serde(default)]
    pub telemetry: TelemetrySection,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wrapper: WrapperSection::default(),
            firecracker: FirecrackerSection::default(),
            network: NetworkSection::default(),
            database: DatabaseSection::default(),
            supervisor_server: SupervisorSection::default(),
            plugins: PluginsSection::default(),
            categories: CategoriesSection::default(),
            deployments: DeploymentsSection::default(),
            binaries: BinariesSection::default(),
            vm: VmSection::default(),
            api: ApiSection::default(),
            telemetry: TelemetrySection::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrapperSection {
    #[serde(default = "default_bind")]
    pub bind_address: String,
    #[serde(default = "default_wrapper_port")]
    pub port: u16,
}

impl Default for WrapperSection {
    fn default() -> Self {
        Self {
            bind_address: default_bind(),
            port: default_wrapper_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirecrackerSection {
    #[serde(default = "default_fc_binary")]
    pub binary_path: String,
    /// Host path to the guest kernel (also the VmRuntime default).
    #[serde(default = "default_kernel_path")]
    pub kernel_path: String,
    /// Host path to the guest initramfs (also the VmRuntime default).
    #[serde(default = "default_initrd_path")]
    pub initrd_path: String,
}

impl Default for FirecrackerSection {
    fn default() -> Self {
        Self {
            binary_path: default_fc_binary(),
            kernel_path: default_kernel_path(),
            initrd_path: default_initrd_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSection {
    #[serde(default = "default_ip_range")]
    pub ip_range: String,
    #[serde(default = "default_gateway")]
    pub gateway: String,
}

impl Default for NetworkSection {
    fn default() -> Self {
        Self {
            ip_range: default_ip_range(),
            gateway: default_gateway(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSection {
    #[serde(default = "default_global_path")]
    pub global_path: String,
    #[serde(default = "default_tenants_dir")]
    pub tenants_dir: String,
}

impl Default for DatabaseSection {
    fn default() -> Self {
        Self {
            global_path: default_global_path(),
            tenants_dir: default_tenants_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorSection {
    #[serde(default = "default_bind")]
    pub bind_address: String,
    #[serde(default = "default_supervisor_port")]
    pub port: u16,
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout_seconds: u64,
}

impl Default for SupervisorSection {
    fn default() -> Self {
        Self {
            bind_address: default_bind(),
            port: default_supervisor_port(),
            heartbeat_timeout_seconds: default_heartbeat_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsSection {
    #[serde(default = "default_plugins_dir")]
    pub directory: String,
    #[serde(default = "default_true")]
    pub hot_reload: bool,
}

impl Default for PluginsSection {
    fn default() -> Self {
        Self {
            directory: default_plugins_dir(),
            hot_reload: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoriesSection {
    #[serde(default = "default_categories_path")]
    pub config_path: String,
}

impl Default for CategoriesSection {
    fn default() -> Self {
        Self {
            config_path: default_categories_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentsSection {
    #[serde(default = "default_deployments_path")]
    pub storage_path: String,
    /// Default TCP port supervisors listen on for deployment pushes (8091).
    #[serde(default = "default_push_port")]
    pub default_push_port: u16,
}

impl Default for DeploymentsSection {
    fn default() -> Self {
        Self {
            storage_path: default_deployments_path(),
            default_push_port: default_push_port(),
        }
    }
}

/// `[binaries]` — host folder with versioned guest binaries + zw assets.
/// Supervisors demand these on boot (version-gated) and on crash redemand.
/// Portal uploads land here instantly; VMs pick them up on next demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinariesSection {
    /// Host dir, e.g. `../binaries` (relative to the wrapper cwd).
    #[serde(default = "default_binaries_dir")]
    pub dir: String,
}

impl Default for BinariesSection {
    fn default() -> Self {
        Self {
            dir: default_binaries_dir(),
        }
    }
}

/// `[vm]` — guest lifecycle: idle auto-sleep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmSection {
    /// Minutes without proxied requests before a VM is shut down.
    /// 0 disables auto-sleep. Shutdown clears the networks row so the next
    /// request re-wakes the VM (wake-on-request round trip).
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_minutes: u64,
    /// Host path of the default agent system prompt (zw
    /// ZS_SYSTEM_PROMPT_FILE source of last resort: tenant row > category
    /// row > global row > this file). Relative paths resolve against the
    /// wrapper's working dir. Empty = probe relative to the disks dir.
    #[serde(default)]
    pub system_prompt_host_path: String,
}

impl Default for VmSection {
    fn default() -> Self {
        Self {
            idle_timeout_minutes: default_idle_timeout(),
            system_prompt_host_path: String::new(),
        }
    }
}

/// `[api]` — public surface for frontends (user UI + admin UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSection {
    /// Public base URL of THIS wrapper as seen from browsers, used to mint
    /// serverless public links: <public_base>/tenant_<id>/<app-path>.
    /// Deployer MUST set this to the host's reachable address.
    #[serde(default = "default_public_base")]
    pub public_base: String,
}

impl Default for ApiSection {
    fn default() -> Self {
        Self {
            public_base: default_public_base(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySection {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_telemetry_output")]
    pub output: String,
    #[serde(default)]
    pub disk: TelemetryDisk,
    #[serde(default)]
    pub broadcast: TelemetryBroadcast,
    #[serde(default)]
    pub metrics: TelemetryMetrics,
    #[serde(default)]
    pub filters: TelemetryFilters,
}

impl Default for TelemetrySection {
    fn default() -> Self {
        Self {
            enabled: true,
            output: default_telemetry_output(),
            disk: TelemetryDisk::default(),
            broadcast: TelemetryBroadcast::default(),
            metrics: TelemetryMetrics::default(),
            filters: TelemetryFilters::default(),
        }
    }
}

impl TelemetrySection {
    /// Convert into the `telemetry` crate's settings type.
    pub fn settings(&self) -> telemetry::TelemetrySettings {
        let mut settings = telemetry::TelemetrySettings::default();
        settings.enabled = self.enabled;
        settings.output = self.output.clone();
        settings.disk.enabled = true;
        settings.disk.path = self.disk.path.clone();
        settings.disk.filename_pattern = self.disk.filename_pattern.clone();
        settings.disk.buffer_size = self.disk.buffer_size;
        settings.disk.flush_interval_seconds = self.disk.flush_interval_seconds;
        settings.disk.compress_old_files = self.disk.compress_old_files;
        settings.disk.retention_days = self.disk.retention_days;
        settings.broadcast.enabled = true;
        settings.broadcast.bind_address = self.broadcast.bind_address.clone();
        settings.broadcast.port = self.broadcast.port;
        settings.broadcast.protocol = self.broadcast.protocol.clone();
        settings.broadcast.max_clients = self.broadcast.max_clients;
        settings.broadcast.events_per_second_per_client =
            self.broadcast.events_per_second_per_client;
        settings.broadcast.auth_required = self.broadcast.auth_required;
        settings.metrics.enabled = self.metrics.enabled;
        settings.metrics.collection_interval_seconds = self.metrics.collection_interval_seconds;
        settings.metrics.track_latency_percentiles =
            self.metrics.track_latency_percentiles.clone();
        settings.filters.exclude_from_disk = self.filters.exclude_from_disk.clone();
        settings.filters.min_level = self.filters.min_level.clone();
        settings
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDisk {
    #[serde(default = "default_telemetry_path")]
    pub path: String,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
    #[serde(default = "default_flush_interval")]
    pub flush_interval_seconds: u64,
    #[serde(default = "default_true")]
    pub compress_old_files: bool,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

impl Default for TelemetryDisk {
    fn default() -> Self {
        Self {
            path: default_telemetry_path(),
            filename_pattern: default_filename_pattern(),
            buffer_size: default_buffer_size(),
            flush_interval_seconds: default_flush_interval(),
            compress_old_files: true,
            retention_days: default_retention_days(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryBroadcast {
    #[serde(default = "default_bind")]
    pub bind_address: String,
    #[serde(default = "default_telemetry_port")]
    pub port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default = "default_max_clients")]
    pub max_clients: usize,
    #[serde(default = "default_eps")]
    pub events_per_second_per_client: u32,
    #[serde(default = "default_true")]
    pub auth_required: bool,
}

impl Default for TelemetryBroadcast {
    fn default() -> Self {
        Self {
            bind_address: default_bind(),
            port: default_telemetry_port(),
            protocol: default_protocol(),
            max_clients: default_max_clients(),
            events_per_second_per_client: default_eps(),
            auth_required: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryMetrics {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_collection_interval")]
    pub collection_interval_seconds: u64,
    #[serde(default = "default_percentiles")]
    pub track_latency_percentiles: Vec<u32>,
}

impl Default for TelemetryMetrics {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_seconds: default_collection_interval(),
            track_latency_percentiles: default_percentiles(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFilters {
    #[serde(default = "default_exclude")]
    pub exclude_from_disk: Vec<String>,
    #[serde(default = "default_min_level")]
    pub min_level: String,
}

impl Default for TelemetryFilters {
    fn default() -> Self {
        Self {
            exclude_from_disk: default_exclude(),
            min_level: default_min_level(),
        }
    }
}

fn default_bind() -> String {
    "0.0.0.0".to_string()
}
fn default_wrapper_port() -> u16 {
    8080
}
fn default_fc_binary() -> String {
    "~/.local/bin/firecracker".to_string()
}
fn default_kernel_path() -> String {
    "../disks/vmlinux.bin".to_string()
}
fn default_initrd_path() -> String {
    "../disks/initramfs.cpio.gz".to_string()
}
fn default_idle_timeout() -> u64 {
    30
}
fn default_public_base() -> String {
    "http://127.0.0.1:8080".to_string()
}
fn default_ip_range() -> String {
    "172.16.0.0/12".to_string()
}
fn default_gateway() -> String {
    "172.16.0.1".to_string()
}
fn default_global_path() -> String {
    "data/global.db".to_string()
}
fn default_tenants_dir() -> String {
    "data/tenants".to_string()
}
fn default_supervisor_port() -> u16 {
    9090
}
fn default_heartbeat_timeout() -> u64 {
    30
}
fn default_plugins_dir() -> String {
    "plugins".to_string()
}
fn default_true() -> bool {
    true
}
fn default_categories_path() -> String {
    "data/categories".to_string()
}
fn default_deployments_path() -> String {
    "data/deployments".to_string()
}
fn default_binaries_dir() -> String {
    "../binaries".to_string()
}
fn default_push_port() -> u16 {
    8091
}
fn default_telemetry_output() -> String {
    "both".to_string()
}
fn default_telemetry_path() -> String {
    "/var/log/wrapper/telemetry".to_string()
}
fn default_filename_pattern() -> String {
    "telemetry_{date}.json".to_string()
}
fn default_buffer_size() -> usize {
    1000
}
fn default_flush_interval() -> u64 {
    5
}
fn default_retention_days() -> u32 {
    30
}
fn default_telemetry_port() -> u16 {
    8081
}
fn default_protocol() -> String {
    "websocket".to_string()
}
fn default_max_clients() -> usize {
    10
}
fn default_eps() -> u32 {
    100
}
fn default_collection_interval() -> u64 {
    10
}
fn default_percentiles() -> Vec<u32> {
    vec![50, 95, 99]
}
fn default_exclude() -> Vec<String> {
    vec!["metric.gauge".to_string(), "metric.counter".to_string()]
}
fn default_min_level() -> String {
    "info".to_string()
}
