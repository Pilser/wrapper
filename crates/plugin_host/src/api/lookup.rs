//! Lookup inter-plugin APIs.

use crate::api::register::PluginApiRegistry;

impl PluginApiRegistry {
    pub fn names(&self) -> Vec<String> {
        self.apis.lock().unwrap().keys().cloned().collect()
    }
}
