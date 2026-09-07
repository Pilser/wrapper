//! A point-in-time value (gauge).

use std::sync::atomic::Ordering;

/// Atomic floating-point gauge.
#[derive(Debug, Default)]
pub struct Gauge(std::sync::atomic::AtomicU64);

impl Gauge {
    pub fn new() -> Self {
        Self(std::sync::atomic::AtomicU64::new(0))
    }

    pub fn set(&self, value: f64) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }

    pub fn value(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::Relaxed))
    }
}