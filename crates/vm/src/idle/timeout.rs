//! Idle timeout configuration.

/// How long a VM may be idle before an action is taken.
#[derive(Debug, Clone)]
pub struct IdleTimeout {
    seconds: u64,
}

impl IdleTimeout {
    pub fn new(seconds: u64) -> Self {
        Self { seconds }
    }

    pub fn as_secs(&self) -> u64 {
        self.seconds
    }
}

impl Default for IdleTimeout {
    fn default() -> Self {
        Self { seconds: 30 * 60 }
    }
}
