//! Aggregation of metrics into a snapshot.

use std::collections::HashMap;

use super::LatencyTracker;

/// A snapshot of the metric registry suitable for dashboards.
#[derive(Debug, Clone, Default)]
pub struct MetricSnapshot {
    pub series: HashMap<String, SeriesSummary>,
}

#[derive(Debug, Clone, Default)]
pub struct SeriesSummary {
    pub samples: usize,
    pub avg_ms: f64,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
    pub p99: Option<f64>,
}

/// Aggregate a set of named latency trackers into a snapshot.
pub fn aggregate(trackers: &HashMap<String, LatencyTracker>) -> MetricSnapshot {
    let mut snap = MetricSnapshot::default();
    for (name, t) in trackers {
        let mut s = SeriesSummary::default();
        if t.count() > 0 {
            s.samples = t.count();
            s.avg_ms = t.avg();
        }
        s.p50 = t.percentile(50);
        s.p95 = t.percentile(95);
        s.p99 = t.percentile(99);
        snap.series.insert(name.clone(), s);
    }
    snap
}