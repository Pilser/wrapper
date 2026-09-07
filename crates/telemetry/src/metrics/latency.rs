//! Latency tracking with percentile computation.

/// Tracks a time series and computes percentiles.
#[derive(Debug, Default)]
pub struct LatencyTracker {
    samples: Vec<f64>,
}

impl LatencyTracker {
    pub fn new() -> Self {
        Self { samples: Vec::new() }
    }

    pub fn observe(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    pub fn percentile(&self, p: u32) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p = p.clamp(0, 100);
        let idx = ((sorted.len().saturating_sub(1)) as f64 * p as f64 / 100.0)
            .round() as usize;
        sorted.get(idx).copied()
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn avg(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }
}