use std::collections::HashMap;

/// Tracks last-seen heartbeat timestamps (unix millis) per tenant.
pub struct HeartbeatTracker {
    timeout_secs: u64,
    last_seen: HashMap<String, i64>,
}

impl HeartbeatTracker {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            timeout_secs,
            last_seen: HashMap::new(),
        }
    }

    pub fn record(&mut self, tenant_id: &str, now_ms: i64) {
        self.last_seen.insert(tenant_id.to_string(), now_ms);
    }

    pub fn last_seen(&self, tenant_id: &str) -> Option<i64> {
        self.last_seen.get(tenant_id).copied()
    }

    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    pub fn remove(&mut self, tenant_id: &str) {
        self.last_seen.remove(tenant_id);
    }
}
