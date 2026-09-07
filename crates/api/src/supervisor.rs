//! Supervisor-server handle for plugins.

pub use supervisor_server::ServerConfig;

/// Thin wrapper exposing supervisor-server config to plugins.
#[derive(Clone, Debug)]
pub struct SupervisorApi {
    pub telemetry: telemetry::Telemetry,
    pub config: ServerConfig,
}

impl SupervisorApi {
    pub fn new(telemetry: telemetry::Telemetry) -> Self {
        Self {
            telemetry,
            config: ServerConfig::default(),
        }
    }

    pub fn with_config(telemetry: telemetry::Telemetry, config: ServerConfig) -> Self {
        Self { telemetry, config }
    }

    /// `host:port` the supervisor listener binds to.
    pub fn addr(&self) -> String {
        self.config.addr()
    }
}
