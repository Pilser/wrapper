//! Upstream connection pool plus health checks.

pub mod connection;
pub mod health;

pub use connection::PooledConn;
pub use health::HealthChecker;

use core::Result;
use std::collections::HashMap;
use telemetry::{Level, Telemetry};

/// Pool of upstream connections keyed by target address.
#[derive(Clone, Debug)]
pub struct ConnectionPool {
    conns: HashMap<String, PooledConn>,
    telemetry: Telemetry,
}

impl ConnectionPool {
    pub fn new(telemetry: Telemetry) -> Self {
        Self {
            conns: HashMap::new(),
            telemetry,
        }
    }

    pub fn insert(&mut self, conn: PooledConn) {
        self.conns.insert(conn.target_addr.clone(), conn);
    }

    pub fn remove(&mut self, target_addr: &str) -> Option<PooledConn> {
        self.conns.remove(target_addr)
    }

    pub fn get(&self, target_addr: &str) -> Option<&PooledConn> {
        self.conns.get(target_addr)
    }

    pub fn len(&self) -> usize {
        self.conns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.conns.is_empty()
    }

    /// Return the pooled connection, verifying it first: entries that fail
    /// the TCP health probe are evicted and re-dialed.
    pub async fn get_or_connect(&mut self, target_addr: &str) -> Result<PooledConn> {
        if let Some(conn) = self.conns.get(target_addr) {
            let checker = HealthChecker::new(self.telemetry.clone());
            if checker.check(conn).await {
                return Ok(conn.clone());
            }
            self.telemetry
                .builder()
                .level(Level::Error)
                .kind("system.health_check.failed")
                .category("proxy")
                .source("proxy::pool")
                .message(format!("evicting unhealthy pooled conn for {target_addr}"))
                .emit();
            self.conns.remove(target_addr);
        }
        if self.conns.len() >= 100 {
            self.telemetry
                .builder()
                .level(Level::Error)
                .kind("proxy.connection.pool_exhausted")
                .category("proxy")
                .source("proxy::pool")
                .message(format!("pool exhausted for {target_addr}"))
                .emit();
        }
        let conn = PooledConn::connect(target_addr, &self.telemetry).await?;
        self.conns.insert(target_addr.to_string(), conn.clone());
        Ok(conn)
    }

    /// Probe every entry and evict the unhealthy ones. Returns evicted count.
    pub async fn evict_unhealthy(&mut self) -> Result<usize> {
        let checker = HealthChecker::new(self.telemetry.clone());
        let addrs: Vec<String> = self.conns.keys().cloned().collect();
        let mut evicted = 0;
        for addr in addrs {
            let healthy = match self.conns.get(&addr) {
                Some(conn) => checker.check(conn).await,
                None => false,
            };
            if !healthy {
                self.conns.remove(&addr);
                evicted += 1;
            }
        }
        if evicted > 0 {
            self.telemetry
                .builder()
                .level(Level::Error)
                .kind("system.health_check.failed")
                .category("proxy")
                .source("proxy::pool")
                .message(format!("evicted {evicted} unhealthy upstream(s)"))
                .emit();
        }
        Ok(evicted)
    }

    /// Count currently healthy entries (probes each one).
    pub async fn health_check(&self) -> Result<usize> {
        let checker = HealthChecker::new(self.telemetry.clone());
        let mut healthy = 0;
        for conn in self.conns.values() {
            if checker.check(conn).await {
                healthy += 1;
            }
        }
        Ok(healthy)
    }
}
