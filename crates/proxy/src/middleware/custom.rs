//! Plugin-supplied custom middleware stub.

use core::Result;
use telemetry::{Level, Telemetry};

/// Named hook holder for plugin middleware.
#[derive(Clone, Debug, Default)]
pub struct CustomMiddleware {
    pub name: String,
    pub hooks: Vec<String>,
}

impl CustomMiddleware {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            hooks: Vec::new(),
        }
    }

    pub fn register_hook(&mut self, hook: impl Into<String>) {
        self.hooks.push(hook.into());
    }

    pub async fn run(&self, telemetry: &Telemetry) -> Result<()> {
        telemetry
            .builder()
            .level(Level::Info)
            .kind("plugin.custom.event")
            .category("plugin")
            .source("proxy::middleware::custom")
            .message(format!("custom middleware {} hooks={}", self.name, self.hooks.len()))
            .emit();
        Ok(())
    }
}
