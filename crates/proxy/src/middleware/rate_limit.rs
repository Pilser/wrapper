//! Fixed-window in-memory rate limiter stub.

use shared::{Error as WrapperError, Result};
use std::collections::HashMap;
use telemetry::{Level, Telemetry};

/// Counts hits per key; rejects past `max_per_minute`.
#[derive(Clone, Debug)]
pub struct RateLimiter {
    pub max_per_minute: u32,
    counts: HashMap<String, u32>,
    telemetry: Telemetry,
}

impl RateLimiter {
    pub fn new(max_per_minute: u32, telemetry: Telemetry) -> Self {
        Self {
            max_per_minute,
            counts: HashMap::new(),
            telemetry,
        }
    }

    pub fn allow(&mut self, key: &str) -> bool {
        let count = self.counts.entry(key.to_string()).or_insert(0);
        *count += 1;
        *count <= self.max_per_minute
    }

    pub fn reset(&mut self, key: &str) {
        self.counts.remove(key);
    }

    pub async fn check(&mut self, key: &str) -> Result<()> {
        if self.allow(key) {
            return Ok(());
        }
        self.telemetry
            .builder()
            .level(Level::Error)
            .kind("auth.login.rate_limited")
            .category("proxy")
            .source("proxy::middleware::rate_limit")
            .message(format!("rate limited key={key}"))
            .emit();
        Err(WrapperError::Proxy(format!("rate limit exceeded for {key}")))
    }
}
