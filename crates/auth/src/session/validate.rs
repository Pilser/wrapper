//! Session validation.

use super::create::SessionStore;

impl SessionStore {
    pub fn is_valid(&self, token: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(token)
    }
}
