//! Per-tenant request activity for idle management.
//!
//! The proxy touches the map on every forwarded request; the wrapper's idle
//! reaper (see `api::server::idle`) shuts down VMs whose tenant has been
//! quiet longer than the configured timeout. Shutdown MUST clear the
//! `networks` row so the next request re-wakes the VM instead of trusting a
//! stale address.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Cloneable handle to the shared tenant -> last-seen map.
#[derive(Clone, Default, Debug)]
pub struct ActivityMap {
    inner: Arc<Mutex<HashMap<String, Instant>>>,
}

impl ActivityMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record "now" as the last activity for `tenant_id`.
    pub fn touch(&self, tenant_id: &str) {
        self.inner
            .lock()
            .unwrap()
            .insert(tenant_id.to_string(), Instant::now());
    }

    /// Last activity, if any.
    pub fn last(&self, tenant_id: &str) -> Option<Instant> {
        self.inner.lock().unwrap().get(tenant_id).copied()
    }

    /// Snapshot of all tracked tenants.
    pub fn snapshot(&self) -> Vec<(String, Instant)> {
        self.inner
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Forget a tenant (called after its VM is shut down).
    pub fn remove(&self, tenant_id: &str) {
        self.inner.lock().unwrap().remove(tenant_id);
    }
}
