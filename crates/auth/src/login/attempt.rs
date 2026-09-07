//! Login attempt tracking (rate limiting).

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct LoginAttempts {
    attempts: Mutex<HashMap<String, u32>>,
}

impl LoginAttempts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, username: &str) -> u32 {
        let mut a = self.attempts.lock().unwrap();
        let n = a.entry(username.to_string()).or_insert(0);
        *n += 1;
        *n
    }

    pub fn is_locked(&self, username: &str, max: u32) -> bool {
        self.attempts.lock().unwrap().get(username).copied().unwrap_or(0) >= max
    }

    pub fn reset(&self, username: &str) {
        self.attempts.lock().unwrap().remove(username);
    }
}
