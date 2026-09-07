//! Spawns one Turso database file per tenant.

use std::collections::HashMap;

use telemetry::Telemetry;
use turso::Builder;

use crate::migration;

/// Manages (and lazily opens) per-tenant database files under `tenants_dir`.
#[derive(Clone)]
pub struct TenantDb {
    pub tenant_id: String,
    pub path: String,
    pub db: turso::Database,
    pub telemetry: Telemetry,
}

impl TenantDb {
    pub async fn open(tenant_id: &str) -> anyhow::Result<Self> {
        Self::open_in("data/tenants", tenant_id).await
    }

    /// Open (creating on first use) the tenant database at
    /// `tenants_dir/<tenant_id>.db`.
    pub async fn open_in(tenants_dir: &str, tenant_id: &str) -> anyhow::Result<Self> {
        let path = format!("{tenants_dir}/{tenant_id}.db");
        std::fs::create_dir_all(tenants_dir)?;
        let db = Builder::new_local(&path).build().await?;
        let conn = db.connect()?;
        migration::tenant::migrate(&conn).await?;
        Ok(Self {
            tenant_id: tenant_id.to_string(),
            path,
            db,
            telemetry: Telemetry::noop(),
        })
    }

    pub fn conn(&self) -> anyhow::Result<turso::Connection> {
        Ok(self.db.connect()?)
    }
}

/// A registry that caches open tenant databases (spawn-many-files).
#[derive(Default)]
pub struct TenantRegistry {
    open: HashMap<String, TenantDb>,
}

impl TenantRegistry {
    pub async fn open(&mut self, tenants_dir: &str, tenant_id: &str) -> anyhow::Result<TenantDb> {
        if let Some(db) = self.open.get(tenant_id) {
            return Ok(db.clone());
        }
        let db = TenantDb::open_in(tenants_dir, tenant_id).await?;
        self.open.insert(tenant_id.to_string(), db.clone());
        Ok(db)
    }

    pub fn close(&mut self, tenant_id: &str) {
        self.open.remove(tenant_id);
    }
}
