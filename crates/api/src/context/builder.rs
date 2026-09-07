//! Fluent builder for [`super::ApiContext`].

use super::ApiContext;
use shared::{Error, Result};

/// Assemble an [`ApiContext`] step by step.
#[derive(Debug, Default)]
pub struct ApiContextBuilder {
    telemetry: Option<telemetry::Telemetry>,
    tenant_id: Option<String>,
    plugin_name: Option<String>,
}

impl ApiContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn telemetry(mut self, telemetry: telemetry::Telemetry) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    pub fn tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    pub fn plugin_name(mut self, name: impl Into<String>) -> Self {
        self.plugin_name = Some(name.into());
        self
    }

    pub fn build(self) -> Result<ApiContext> {
        let telemetry = self
            .telemetry
            .ok_or_else(|| Error::InvalidInput("telemetry handle is required".into()))?;
        let plugin_name = self
            .plugin_name
            .ok_or_else(|| Error::InvalidInput("plugin_name is required".into()))?;
        Ok(ApiContext {
            telemetry,
            tenant_id: self.tenant_id,
            plugin_name,
        })
    }
}
