//! `db` — global and per-tenant databases backed by Turso.
//!
//! The wrapper owns all data. This crate opens the GLOBAL database (tenants,
//! sessions, categories, vm_templates, networks, deployment configs) and can
//! spawn any number of PER-TENANT database files via `turso::Builder::new_local`.

pub mod global;
pub mod migration;
pub mod query;
pub mod tenant;

pub use global::{GlobalDb, GlobalStore};
pub use tenant::TenantDb;

/// A facade exposing both the global store and per-tenant stores.
#[derive(Clone)]
pub struct Db {
    pub global: GlobalDb,
}

impl Db {
    pub async fn open(global_path: &str, tenants_dir: &str) -> anyhow::Result<Self> {
        let global = GlobalDb::open(global_path).await?;
        let store = GlobalStore { db: global.clone() };
        let _ = store;
        let _ = tenants_dir;
        Ok(Self { global })
    }

    pub fn global(&self) -> &GlobalDb {
        &self.global
    }

    pub async fn tenant(&self, tenant_id: &str) -> anyhow::Result<TenantDb> {
        TenantDb::open(tenant_id).await
    }
}

/// Current RFC3339 timestamp.
pub fn now_str() -> String {
    shared::now_rfc3339()
}
