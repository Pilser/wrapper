//! In-process metric registry (counters, gauges, histograms, latency).

pub mod aggregate;
pub mod counter;
pub mod gauge;
pub mod histogram;
pub mod latency;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use counter::Counter;
pub use gauge::Gauge;
pub use histogram::Histogram;
pub use latency::LatencyTracker;

/// Tracks aggregated counters, gauges, histograms and event counts.
#[derive(Clone, Default)]
pub struct Metrics {
    counts: Arc<Mutex<HashMap<String, u64>>>,
    latencies: Arc<Mutex<HashMap<String, Vec<f64>>>>,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a raw sample for a metric name.
    pub fn sample(&self, name: &str, value: f64) {
        let mut l = self.latencies.lock().unwrap();
        l.entry(name.to_string()).or_default().push(value);
    }

    /// Record that an event of type `kind` fired (optionally per trace).
    pub fn record_event(&self, kind: String, _trace: Option<String>) {
        // Aggregate by top-level category segment, e.g. `vm.boot.started` -> `vm`.
        let category = kind.split('.').next().unwrap_or("system").to_string();
        let mut c = self.counts.lock().unwrap();
        *c.entry(format!("event.{category}")).or_insert(0) += 1;
    }

    /// Snapshot counters as map.
    pub fn counters(&self) -> HashMap<String, u64> {
        self.counts.lock().unwrap().clone()
    }

    /// Snapshot latency samples as map.
    pub fn latencies(&self) -> HashMap<String, Vec<f64>> {
        self.latencies.lock().unwrap().clone()
    }
}

impl std::fmt::Debug for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Metrics({} counters, {} series)", self.counters().len(), self.latencies().len())
    }
}