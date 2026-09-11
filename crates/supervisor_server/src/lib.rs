pub mod deployment;
pub mod heartbeat;
pub mod listener;
pub mod mcp;
pub mod protocol;
pub mod tools;

pub use protocol::message::Envelope;

use shared::{Error, Result};

/// Configuration for the supervisor TCP listener.
/// Mirrors `[supervisor_server]` in `config.toml`.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_address: String,
    pub port: u16,
    pub heartbeat_timeout_seconds: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 9090,
            heartbeat_timeout_seconds: 30,
        }
    }
}

impl ServerConfig {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.bind_address, self.port)
    }

    pub async fn start(
        &self,
        telemetry: telemetry::Telemetry,
        db: db::Db,
        auth: auth::AuthService,
        activity: proxy::ActivityMap,
    ) -> Result<()> {
        listener::tcp::serve(&self.addr(), telemetry, db, auth, activity).await
    }
}

/// Every supervisor message must carry a non-empty tenant id.
pub fn validate_tenant_id(tenant_id: &str) -> Result<()> {
    if tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput("missing tenant_id".to_string()));
    }
    Ok(())
}
