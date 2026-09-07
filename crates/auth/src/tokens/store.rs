//! Token manager: mint, verify, revoke.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::generate::mint_token;

#[derive(Clone, Default)]
pub struct TokenManager {
    pub(crate) tokens: Arc<Mutex<HashMap<String, String>>>, // token -> subject
}

impl TokenManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mint(&self, subject: &str) -> anyhow::Result<String> {
        let token = mint_token(subject);
        self.tokens.lock().unwrap().insert(token.clone(), subject.to_string());
        Ok(token)
    }

    pub fn verify(&self, token: &str) -> Option<String> {
        self.tokens.lock().unwrap().get(token).cloned()
    }

    pub fn revoke(&self, token: &str) -> anyhow::Result<()> {
        self.tokens.lock().unwrap().remove(token);
        Ok(())
    }

    pub fn is_valid(&self, token: &str) -> bool {
        self.tokens.lock().unwrap().contains_key(token)
    }
}
