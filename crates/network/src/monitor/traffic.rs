//! Traffic monitor reading /proc/net/dev deltas.

use std::collections::HashMap;
use std::sync::Mutex;

use telemetry::Telemetry;

/// Read (rx_bytes, tx_bytes) for each interface from /proc/net/dev.
pub fn read_bytes_per_iface() -> anyhow::Result<HashMap<String, (u64, u64)>> {
    let content = std::fs::read_to_string("/proc/net/dev")?;
    let mut out = HashMap::new();
    for line in content.lines().skip(2) {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let name = name.trim_end_matches(':').to_string();
        let Some(rx) = parts.next() else { continue };
        let fields: Vec<&str> = parts.collect();
        let tx = fields.get(7).and_then(|v| v.parse().ok()).unwrap_or(0);
        out.insert(name, (rx.parse().unwrap_or(0), tx));
    }
    Ok(out)
}

/// Snapshot of traffic per interface.
#[derive(Default)]
pub struct TrafficMonitor {
    last: Mutex<HashMap<String, (u64, u64)>>,
}

impl Clone for TrafficMonitor {
    fn clone(&self) -> Self {
        Self {
            last: Mutex::new(self.last.lock().unwrap().clone()),
        }
    }
}

impl TrafficMonitor {
    pub fn with_telemetry(_telemetry: Telemetry) -> Self {
        Self::default()
    }

    pub fn bandwidth(&self, iface: &str) -> anyhow::Result<super::Bandwidth> {
        let map = read_bytes_per_iface()?;
        let current = map.get(iface).copied().unwrap_or((0, 0));
        let mut last = self.last.lock().unwrap();
        let prev = last.get(iface).copied().unwrap_or_default();
        last.insert(iface.to_string(), current);
        Ok(super::Bandwidth {
            iface: iface.to_string(),
            rx_bytes_delta: current.0.saturating_sub(prev.0),
            tx_bytes_delta: current.1.saturating_sub(prev.1),
            bandwidth_rate_bytes: 0,
        })
    }

    pub fn rate_for(&self, iface: &str) -> anyhow::Result<u64> {
        Ok(self.bandwidth(iface)?.bandwidth_rate_bytes)
    }
}
