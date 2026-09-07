//! Network handle for plugins (delegates to the `network` crate).

use shared::{Error, Result};

/// Thin wrapper exposing virtual networking to plugins.
#[derive(Clone, Debug)]
pub struct NetworkApi {
    pub telemetry: telemetry::Telemetry,
}

impl NetworkApi {
    pub fn new(telemetry: telemetry::Telemetry) -> Self {
        Self { telemetry }
    }

    /// Build the underlying [`network::NetworkManager`].
    pub fn manager(&self, cfg: shared::NetworkConfig) -> Result<network::NetworkManager> {
        network::NetworkManager::with_telemetry(cfg, self.telemetry.clone())
            .map_err(|e| Error::Network(e.to_string()))
    }
}
