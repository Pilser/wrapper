//! Register inter-plugin APIs.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

pub type ApiFn = Box<dyn Fn(&Value) -> Value + Send + Sync>;

#[derive(Default)]
pub struct PluginApiRegistry {
    pub(crate) apis: Mutex<HashMap<String, ApiFn>>,
}

impl PluginApiRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, name: impl Into<String>, f: ApiFn) {
        self.apis.lock().unwrap().insert(name.into(), f);
    }

    pub fn call(&self, name: &str, args: &Value) -> Option<Value> {
        self.apis.lock().unwrap().get(name).map(|f| f(args))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.apis.lock().unwrap().contains_key(name)
    }
}
