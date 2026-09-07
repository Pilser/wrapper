//! Session store.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use core::Session;

#[derive(Clone, Default)]
pub struct SessionStore {
    pub(crate) sessions: Arc<Mutex<HashMap<String, Session>>>, // token -> session
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, username: &str, token: &str) -> anyhow::Result<()> {
        let now = core::now_rfc3339();
        let session = Session {
            id: format!("sess_{}", &token[..8]),
            tenant_id: username.to_string(),
            token: token.to_string(),
            ip_address: None,
            created_at: now.clone(),
            expires_at: now.clone(),
            last_active: now,
        };
        self.sessions.lock().unwrap().insert(token.to_string(), session);
        Ok(())
    }

    pub fn get(&self, token: &str) -> Option<Session> {
        self.sessions.lock().unwrap().get(token).cloned()
    }

    pub fn destroy(&self, token: &str) -> anyhow::Result<()> {
        self.sessions.lock().unwrap().remove(token);
        Ok(())
    }

    pub fn list(&self) -> Vec<Session> {
        self.sessions.lock().unwrap().values().cloned().collect()
    }
}
