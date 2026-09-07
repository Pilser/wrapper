//! Pre-computed pool of tenant IPs with a mutex-protected free set.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use shared::NetworkConfig;
use telemetry::Telemetry;

/// A pool of allocatable tenant IPs drawn from `[network] ip_range`.
pub struct IpPool {
    gateway: String,
    pub(crate) free: Mutex<HashSet<String>>,
    pub(crate) taken: Mutex<HashMap<String, String>>, // tenant_id -> ip
    telemetry: Telemetry,
}

impl Clone for IpPool {
    fn clone(&self) -> Self {
        IpPool {
            gateway: self.gateway.clone(),
            free: Mutex::new(self.free.lock().unwrap().clone()),
            taken: Mutex::new(self.taken.lock().unwrap().clone()),
            telemetry: self.telemetry.clone(),
        }
    }
}

impl IpPool {
    pub fn new(cfg: NetworkConfig) -> anyhow::Result<Self> {
        Ok(Self::with_telemetry(cfg, Telemetry::noop()))
    }

    pub fn with_telemetry(cfg: NetworkConfig, telemetry: Telemetry) -> Self {
        let mut free = HashSet::new();
        // Use the third octet prefix of the configured range (e.g. 172.16.0.0/12 -> 172.16.0).
        let prefix = cfg.ip_range.split('/').next().unwrap_or("172.16.0").to_string();
        let start = 2u32;
        let end = start + cfg.max_tenants.min(65_534) as u32 - 1;
        for n in start..=end {
            free.insert(format!("{prefix}.{n}"));
        }
        Self {
            gateway: cfg.gateway,
            free: Mutex::new(free),
            taken: Mutex::new(HashMap::new()),
            telemetry,
        }
    }

    pub fn available(&self) -> usize {
        self.free.lock().unwrap().len()
    }

    pub fn gateway(&self) -> &str {
        &self.gateway
    }

    /// The shared telemetry handle for this pool.
    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }
}
