//! Periodic session cleanup.

use super::create::SessionStore;

impl SessionStore {
    pub fn cleanup(&self) -> usize {
        self.purge_expired(&core::now_rfc3339())
    }
}
