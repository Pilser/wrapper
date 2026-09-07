//! Token verification helpers.

use super::store::TokenManager;

impl TokenManager {
    pub fn subject_of(&self, token: &str) -> Option<String> {
        self.verify(token)
    }
}
