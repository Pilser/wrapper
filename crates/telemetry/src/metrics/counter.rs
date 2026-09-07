//! A monotonically increasing counter.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Counter(AtomicU64);

impl Counter {
    pub fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    pub fn inc(&self) {
        self.add(1);
    }

    pub fn add(&self, n: u64) {
        self.0.fetch_add(n, Ordering::Relaxed);
    }

    pub fn value(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}