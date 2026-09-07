//! Priority metadata for hook subscriptions.

pub struct Priority {
    pub order: u32,
}

impl Priority {
    pub fn default() -> Self {
        Self { order: 1 }
    }
}
