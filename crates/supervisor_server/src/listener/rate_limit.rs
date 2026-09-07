use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Simple per-key sliding-window rate limiter (per minute).
pub struct RateLimiter {
    max_per_minute: u32,
    hits: HashMap<String, VecDeque<Instant>>,
}

impl RateLimiter {
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            max_per_minute,
            hits: HashMap::new(),
        }
    }

    pub fn allow(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let queue = self.hits.entry(key.to_string()).or_default();
        while let Some(front) = queue.front() {
            if now.duration_since(*front) > window {
                queue.pop_front();
            } else {
                break;
            }
        }
        if queue.len() as u32 >= self.max_per_minute {
            return false;
        }
        queue.push_back(now);
        true
    }

    pub fn reset(&mut self, key: &str) {
        self.hits.remove(key);
    }
}
