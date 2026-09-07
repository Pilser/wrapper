use core::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-over-TCP envelope. Every wrapper <-> supervisor message carries
/// `{type, tenant_id, timestamp, payload}` (see IMPLEMENTATION-GUIDE.txt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub tenant_id: String,
    pub timestamp: String,
    #[serde(default)]
    pub payload: Value,
}

impl Envelope {
    pub fn new(
        msg_type: impl Into<String>,
        tenant_id: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            msg_type: msg_type.into(),
            tenant_id: tenant_id.into(),
            timestamp: core::now_rfc3339(),
            payload,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.msg_type.trim().is_empty() {
            return Err(Error::InvalidInput("message missing `type`".to_string()));
        }
        if self.tenant_id.trim().is_empty() {
            return Err(Error::InvalidInput("message missing `tenant_id`".to_string()));
        }
        if self.timestamp.trim().is_empty() {
            return Err(Error::InvalidInput("message missing `timestamp`".to_string()));
        }
        Ok(())
    }

    /// Parse one newline-delimited JSON message.
    pub fn from_line(line: &str) -> Result<Self> {
        let env: Self = serde_json::from_str(line)?;
        env.validate()?;
        Ok(env)
    }

    /// Serialize to one line (no trailing newline).
    pub fn to_line(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Build a response that carries over the tenant id.
    pub fn respond(&self, msg_type: impl Into<String>, payload: Value) -> Self {
        Self::new(msg_type, self.tenant_id.clone(), payload)
    }
}
