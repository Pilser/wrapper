//! Invoke a registered API with JSON args.

use serde_json::Value;

use super::register::PluginApiRegistry;

impl PluginApiRegistry {
    pub fn invoke(&self, name: &str, args: &Value) -> Option<Value> {
        self.call(name, args)
    }
}
