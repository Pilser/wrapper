//! Stores loaded plugins by name.

use std::collections::HashMap;

use crate::loader::{LoadOutcome, unload_plugin};

#[derive(Default)]
pub struct PluginRegistry {
    pub(crate) plugins: HashMap<String, LoadOutcome>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, name: String, plugin: LoadOutcome) {
        self.plugins.insert(name, plugin);
    }

    pub fn remove(&mut self, name: &str) -> Option<LoadOutcome> {
        self.plugins.remove(name)
    }

    pub fn get(&self, name: &str) -> Option<&LoadOutcome> {
        self.plugins.get(name)
    }

    pub fn names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Unload all plugins (used at shutdown).
    pub fn unload_all(&mut self) {
        let names: Vec<String> = self.names();
        for name in names {
            let _ = unload_plugin(&self.plugins.remove(&name).unwrap(), &telemetry::Telemetry::noop());
        }
    }
}
