//! Upstream VM address lookup backed by the global `networks` table.
//!
//! Resolution order for a tenant:
//! 1. If a [`db::global::GlobalDb`] handle was attached with
//!    [`VmLookup::with_db`], query the `networks` table (via the real
//!    [`db::global::networks`] functions) for a row matching the tenant id.
//!    A bare IP from the table gets [`DEFAULT_AGENT_PORT`] appended; a value
//!    that already contains a port is used as-is.
//! 2. Otherwise — no DB handle, no matching row, or a query error — fall back
//!    to the configured default target (operator config default).

use shared::{Error as WrapperError, Result};
use telemetry::{Level, Telemetry};

/// VM agent port appended to bare IPs read from the `networks` table.
pub const DEFAULT_AGENT_PORT: u16 = 9000;

/// Fallback target used when the DB has no row for the tenant.
pub const DEFAULT_TARGET: &str = "127.0.0.1:9000";

/// Looks up the VM/agent address for a tenant.
///
/// Attach the global DB with [`VmLookup::with_db`] (typically right after
/// [`db::global::GlobalDb::open`]) so lookups hit the `networks` table.
/// Without a DB handle every lookup returns the default target.
#[derive(Clone)]
pub struct VmLookup {
    telemetry: Telemetry,
    default_target: String,
    agent_port: u16,
    db: Option<db::global::GlobalDb>,
}

impl std::fmt::Debug for VmLookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmLookup")
            .field("default_target", &self.default_target)
            .field("agent_port", &self.agent_port)
            .field("has_db", &self.db.is_some())
            .finish()
    }
}

impl VmLookup {
    pub fn new(telemetry: Telemetry) -> Self {
        Self {
            telemetry,
            default_target: DEFAULT_TARGET.to_string(),
            agent_port: DEFAULT_AGENT_PORT,
            db: None,
        }
    }

    /// Override the fallback target (operator config default).
    pub fn with_default(mut self, default_target: impl Into<String>) -> Self {
        self.default_target = default_target.into();
        self
    }

    /// Override the agent port appended to bare IPs from the DB.
    pub fn with_agent_port(mut self, port: u16) -> Self {
        self.agent_port = port;
        self
    }

    /// Attach the global DB so lookups query the `networks` table.
    pub fn with_db(mut self, db: db::global::GlobalDb) -> Self {
        self.db = Some(db);
        self
    }

    pub fn default_target(&self) -> &str {
        &self.default_target
    }

    /// Resolve `tenant_id` to a `host:port` upstream address.
    ///
    /// Always succeeds: falls back to the default target when the DB has no
    /// row for the tenant (or no DB is attached / the query fails).
    pub async fn lookup(&self, tenant_id: &str) -> Result<String> {
        if let Some(db) = &self.db {
            match self.lookup_db(db, tenant_id).await {
                Ok(Some(addr)) => {
                    self.telemetry
                        .builder()
                        .level(Level::Info)
                        .kind("proxy.request.forwarded")
                        .category("proxy")
                        .source("proxy::routing::vm_lookup")
                        .message(format!("vm lookup tenant={tenant_id} addr={addr} source=db"))
                        .emit();
                    return Ok(addr);
                }
                Ok(None) => {
                    self.telemetry
                        .builder()
                        .level(Level::Info)
                        .kind("proxy.request.forwarded")
                        .category("proxy")
                        .source("proxy::routing::vm_lookup")
                        .message(format!(
                            "vm lookup tenant={tenant_id} has no networks row, using default {}",
                            self.default_target
                        ))
                        .emit();
                }
                Err(err) => {
                    self.telemetry
                        .builder()
                        .level(Level::Error)
                        .kind("proxy.request.failed")
                        .category("proxy")
                        .source("proxy::routing::vm_lookup")
                        .message(format!(
                            "vm lookup tenant={tenant_id} db error: {err}, using default {}",
                            self.default_target
                        ))
                        .emit();
                }
            }
        } else {
            self.telemetry
                .builder()
                .level(Level::Info)
                .kind("proxy.request.forwarded")
                .category("proxy")
                .source("proxy::routing::vm_lookup")
                .message(format!(
                    "vm lookup tenant={tenant_id} no db attached, using default {}",
                    self.default_target
                ))
                .emit();
        }
        Ok(self.default_target.clone())
    }

    /// Query the `networks` table for this tenant's IP.
    async fn lookup_db(
        &self,
        db: &db::global::GlobalDb,
        tenant_id: &str,
    ) -> Result<Option<String>> {
        let conn = db
            .conn()
            .map_err(|e| WrapperError::Database(e.to_string()))?;
        let rows = db::global::networks::all(&conn)
            .await
            .map_err(|e| WrapperError::Database(e.to_string()))?;
        Ok(rows
            .into_iter()
            .find(|row| row.tenant_id == tenant_id)
            .map(|row| self.addr_of(&row.ip_address)))
    }

    /// Turn a stored IP into a `host:port` address.
    fn addr_of(&self, ip: &str) -> String {
        let ip = ip.trim();
        if ip.contains(':') {
            ip.to_string()
        } else {
            format!("{ip}:{}", self.agent_port)
        }
    }
}
