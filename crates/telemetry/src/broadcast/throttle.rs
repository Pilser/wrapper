//! Client-side rate limiting for the live stream.

/// A simple fixed-window limiter that allows up to `events_per_second` events
/// per second.
#[derive(Debug)]
pub struct RateLimiter {
    window: std::time::Duration,
    window_start: std::time::Instant,
    count: u32,
    limit: u32,
}

impl RateLimiter {
    pub fn new(events_per_second: u32) -> Self {
        Self {
            window: std::time::Duration::from_secs(1),
            window_start: std::time::Instant::now(),
            count: 0,
            limit: events_per_second.max(1),
        }
    }

    /// Returns true if the event may be sent.
    pub fn allow(&mut self) -> bool {
        if self.window_start.elapsed() >= self.window {
            self.window_start = std::time::Instant::now();
            self.count = 0;
        }
        if self.count < self.limit {
            self.count += 1;
            true
        } else {
            false
        }
    }
}