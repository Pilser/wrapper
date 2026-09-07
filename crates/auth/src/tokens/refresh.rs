//! Token refresh.

use super::store::TokenManager;

impl TokenManager {
    /// Replace an old token with a fresh one for the same subject.
    pub fn refresh(&self, old_token: &str) -> anyhow::Result<Option<String>> {
        let subject = match self.verify(old_token) {
            Some(s) => s,
            None => return Ok(None),
        };
        self.revoke(old_token)?;
        self.mint(&subject).map(Some)
    }
}
