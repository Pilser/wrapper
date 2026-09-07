//! Session expiry.

use super::create::SessionStore;

impl SessionStore {
    pub fn expire(&self, token: &str) -> anyhow::Result<()> {
        self.destroy(token)
    }

    pub fn purge_expired(&self, _now: &str) -> usize {
        // In a real impl compare expires_at to now.
        0
    }
}
