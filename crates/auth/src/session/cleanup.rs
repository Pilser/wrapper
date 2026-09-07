//! Periodic session cleanup.

use super::create::SessionStore;

impl SessionStore {
    pub fn cleanup(&self) -> usize {
        self.purge_expired(&shared::now_rfc3339())
    }
}
