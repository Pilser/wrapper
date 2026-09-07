//! Monitors per-VM resource usage.

use std::collections::HashMap;
use std::sync::Mutex;

use super::limits::ResourceLimits;

#[derive(Default)]
pub struct ResourceMonitor {
    usage: Mutex<HashMap<String, ResourceSample>>,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, vm_id: &str, sample: ResourceSample) {
        self.usage.lock().unwrap().insert(vm_id.to_string(), sample);
    }

    pub fn sample(&self, vm_id: &str) -> Option<ResourceSample> {
        self.usage.lock().unwrap().get(vm_id).cloned()
    }

    pub fn exceeds(&self, vm_id: &str, limits: &ResourceLimits) -> bool {
        self.usage.lock().unwrap().get(vm_id).map(|s| s.exceeds(limits)).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResourceSample {
    pub cpu_percent: f64,
    pub memory_mb: u32,
    pub disk_mb: u32,
}

impl ResourceSample {
    pub fn new(cpu: f64, mem_mb: u32, disk_mb: u32) -> Self {
        Self { cpu_percent: cpu, memory_mb: mem_mb, disk_mb: disk_mb }
    }

    pub fn exceeds(&self, limits: &ResourceLimits) -> bool {
        self.cpu_percent > limits.max_cpu_percent
            || self.memory_mb > limits.max_memory_mb
            || self.disk_mb > limits.max_disk_mb
    }
}
