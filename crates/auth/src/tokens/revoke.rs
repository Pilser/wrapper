//! Token revocation.

use super::store::TokenManager;

impl TokenManager {
    pub fn revoke_all(&self, subject: &str) -> anyhow::Result<()> {
        let mut guard = self.tokens.lock().unwrap();
        let to_remove: Vec<String> = guard.iter().filter(|(_, s)| *s == subject).map(|(t, _)| t.clone()).collect();
        for t in to_remove {
            guard.remove(&t);
        }
        Ok(())
    }
}
