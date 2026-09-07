//! Lookup helpers over the registry.

use crate::loader::LoadOutcome;
use crate::registry::store::PluginRegistry;

impl PluginRegistry {
    pub fn by_name(&self, name: &str) -> Option<&LoadOutcome> {
        self.plugins.get(name)
    }

    pub fn plugin_type(&self, name: &str) -> Option<core::PluginType> {
        self.plugins.get(name).map(|p| p.info.plugin_type)
    }
}
