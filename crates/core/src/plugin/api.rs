//! The plugin-facing API surface.
//!
//! This struct is intentionally a thin, cloneable token in `core`. The `api`
//! crate extends it with concrete handles (db, vm, network, telemetry) by
//! keeping its own runtime registry keyed on the plugin id, so `core` never
//! has to know about the subsystems that plugin code will touch.

/// A plugin-scoped runtime handle.
#[derive(Debug, Clone, Default)]
pub struct PluginApi {
    /// Unique id assigned to the plugin instance this API belongs to.
    pub plugin_id: String,
    /// Arbitrary runtime extensions registered by the `api` crate.
    pub extensions: std::collections::HashMap<String, serde_json::Value>,
}

impl PluginApi {
    pub fn new(plugin_id: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            extensions: Default::default(),
        }
    }

    pub fn extend(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.extensions.insert(key.into(), value);
    }
}