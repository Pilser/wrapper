//! TLS mode reporting for the proxy.
//!
//! The proxy serves plain TCP (hyper HTTP/1.1 plus connection upgrades) and
//! does NOT perform in-process TLS termination: there is deliberately no
//! `rustls`/`tokio-rustls` accept path in this crate (neither is a workspace
//! dependency). Terminate TLS at a load balancer or reverse proxy in front of
//! the wrapper and forward plain HTTP to the proxy listener.
//!
//! [`TlsConfig`] records the operator's intent: `disabled()` is the only
//! serving mode; a config carrying cert/key paths is rejected loudly by
//! [`TlsConfig::describe`] instead of silently serving plaintext.

use shared::{Error as WrapperError, Result};
use telemetry::{Level, Telemetry};

/// TLS settings. `None` paths mean plain-TCP mode (the only serving mode).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TlsConfig {
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl TlsConfig {
    pub fn disabled() -> Self {
        Self {
            cert_path: None,
            key_path: None,
        }
    }

    pub fn new(cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        Self {
            cert_path: Some(cert_path.into()),
            key_path: Some(key_path.into()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.cert_path.is_some() && self.key_path.is_some()
    }

    /// Report the effective serving mode.
    ///
    /// Plain TCP always: `Ok` when TLS is disabled, `Err` when cert paths are
    /// configured (in-process termination is unsupported — terminate upstream).
    pub async fn describe(&self, telemetry: &Telemetry) -> Result<String> {
        if !self.is_enabled() {
            return Ok("tls disabled: serving plain TCP (terminate TLS upstream)".to_string());
        }
        let cert = self.cert_path.clone().unwrap_or_default();
        telemetry
            .builder()
            .level(Level::Error)
            .kind("proxy.request.failed")
            .category("proxy")
            .source("proxy::server::tls")
            .message(format!(
                "tls requested cert={cert} but in-process termination is unsupported"
            ))
            .emit();
        if cert.is_empty() {
            return Err(WrapperError::Proxy("tls cert path is empty".to_string()));
        }
        Err(WrapperError::Proxy(format!(
            "tls termination is not supported in-process (cert={cert}); \
             terminate TLS upstream and use plain TCP"
        )))
    }
}
