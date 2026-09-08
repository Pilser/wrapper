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

    /// Allocate a free IP for `tenant_id`. Returns None if the pool is exhausted
    /// or the tenant already holds an address.
    pub async fn allocate(&self, tenant_id: &str) -> anyhow::Result<Option<String>> {
        let mut free = self.free.lock().unwrap();
        let mut taken = self.taken.lock().unwrap();
        if taken.contains_key(tenant_id) {
            return Ok(taken.get(tenant_id).cloned());
        }
        let ip = free.iter().next().cloned();
        if let Some(ip) = ip {
            free.remove(&ip);
            taken.insert(tenant_id.to_string(), ip.clone());
            self.telemetry
                .builder()
                .level(telemetry::Level::Info)
                .kind("network.ip.allocated")
                .category("network")
                .tenant_id(Some(tenant_id.to_string()))
                .message(format!("allocated {ip} to {tenant_id}"))
                .data("ip", ip.clone())
                .emit();
            Ok(Some(ip))
        } else {
            Ok(None)
        }
    }

    /// Release a tenant's IP back to the free pool.
    pub async fn release(&self, tenant_id: &str) -> anyhow::Result<()> {
        let mut free = self.free.lock().unwrap();
        let mut taken = self.taken.lock().unwrap();
        if let Some(ip) = taken.remove(tenant_id) {
            free.insert(ip);
        }
        Ok(())
    }

    /// The shared telemetry handle for this pool.
    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }
}
