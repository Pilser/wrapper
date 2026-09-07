//! Proxy handle for plugins (re-exports `proxy` hook registry).

pub use proxy::ProxyHooks;

/// Thin wrapper exposing the proxy hook points to plugins.
#[derive(Clone, Debug, Default)]
pub struct ProxyApi {
    pub hooks: ProxyHooks,
}

impl ProxyApi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hooks(&self) -> &ProxyHooks {
        &self.hooks
    }
}
