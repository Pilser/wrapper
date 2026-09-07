//! Handed to a plugin at register/unregister time so it can reach the system.

use serde_json::Value;

use super::{PluginApi, PluginInfo};

/// Everything a plugin needs to interact with the wrapper at runtime.
///
/// The `api` handle is the plugin-facing façade assembled by the `api` crate;
/// individual crates may add typed handles (db, vm, ...) onto it later without
/// changing this struct, via extension fields in [`PluginApi`].
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// The metadata under which this plugin was registered.
    pub info: PluginInfo,
    /// Plugin-facing handles into the wrapper subsystems.
    pub api: PluginApi,
    /// Optional per-plugin configuration blob (free-form JSON).
    pub config: Value,
}

impl PluginContext {
    pub fn new(info: PluginInfo, api: PluginApi) -> Self {
        Self {
            info,
            api,
            config: Value::Object(Default::default()),
        }
    }

    pub fn with_config(mut self, config: Value) -> Self {
        self.config = config;
        self
    }
}