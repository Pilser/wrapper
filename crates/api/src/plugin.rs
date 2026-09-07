//! Plugin-host handle for plugins (re-exports the `core` plugin contract).

pub use shared::{HookPoint, Plugin, PluginContext, PluginInfo};

/// Thin wrapper exposing plugin metadata to plugins.
#[derive(Clone, Debug)]
pub struct PluginApi {
    pub telemetry: telemetry::Telemetry,
}

impl PluginApi {
    pub fn new(telemetry: telemetry::Telemetry) -> Self {
        Self { telemetry }
    }

    /// Describe this API surface as plugin info.
    pub fn info(&self) -> PluginInfo {
        let mut info = PluginInfo::new("wrapper-api");
        info.version = shared::PluginVersion::new(0, 1, 0);
        info.plugin_type = shared::PluginType::Builtin;
        info.description = "wrapper plugin API".to_string();
        info
    }
}
