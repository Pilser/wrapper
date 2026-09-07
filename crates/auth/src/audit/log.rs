//! Append-only audit log.

use std::sync::Mutex;

#[derive(Default)]
pub struct AuditLog {
    entries: Mutex<Vec<String>>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, entry: impl Into<String>) {
        self.entries.lock().unwrap().push(entry.into());
    }

    pub fn entries(&self) -> Vec<String> {
        self.entries.lock().unwrap().clone()
    }
}
