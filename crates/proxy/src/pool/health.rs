//! Upstream connection health checks via TCP connect probes.

use shared::Result;
use telemetry::{Level, Telemetry};

/// Probes pooled upstreams; unhealthy entries are evicted by the pool.
#[derive(Clone, Debug)]
pub struct HealthChecker {
    telemetry: Telemetry,
}

impl HealthChecker {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    /// Probe one connection: fresh TCP dial to its target address.
    pub async fn check(&self, conn: &crate::pool::PooledConn) -> bool {
        if conn.probe().await {
            self.telemetry
                .builder()
                .level(Level::Info)
                .kind("system.health_check.passed")
                .category("proxy")
                .source("proxy::pool::health")
                .message(format!("healthy upstream {}", conn.target_addr))
                .emit();
            true
        } else {
            self.telemetry
                .builder()
                .level(Level::Error)
                .kind("system.health_check.failed")
                .category("proxy")
                .source("proxy::pool::health")
                .message(format!("unhealthy upstream {}", conn.target_addr))
                .emit();
            false
        }
    }

    pub async fn check_all(&self, conns: &[crate::pool::PooledConn]) -> Result<usize> {
        let mut healthy = 0;
        for conn in conns {
            if self.check(conn).await {
                healthy += 1;
            }
        }
        Ok(healthy)
    }
}
