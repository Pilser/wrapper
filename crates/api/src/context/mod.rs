//! Handle passed to plugins: identity + telemetry.

pub mod access;
pub mod builder;

pub use builder::ApiContextBuilder;

/// The context a plugin receives on every call.
#[derive(Clone, Debug)]
pub struct ApiContext {
    pub telemetry: telemetry::Telemetry,
    pub tenant_id: Option<String>,
    pub plugin_name: String,
}

impl ApiContext {
    pub fn new(telemetry: telemetry::Telemetry, plugin_name: impl Into<String>) -> Self {
        Self {
            telemetry,
            tenant_id: None,
            plugin_name: plugin_name.into(),
        }
    }

    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }
}
