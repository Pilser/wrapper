//! Per-VM resource limits.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_cpu_percent: f64,
    pub max_memory_mb: u32,
    pub max_disk_mb: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self { max_cpu_percent: 80.0, max_memory_mb: 2048, max_disk_mb: 10_240 }
    }
}

impl ResourceLimits {
    pub fn new(cpu: f64, mem_mb: u32, disk_mb: u32) -> Self {
        Self { max_cpu_percent: cpu, max_memory_mb: mem_mb, max_disk_mb: disk_mb }
    }
}
