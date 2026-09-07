//! Database handle for plugins (delegates to the `db` crate).

use core::{Error, Result};

/// Thin wrapper exposing the global database to plugins.
#[derive(Clone, Debug)]
pub struct DbApi {
    pub telemetry: telemetry::Telemetry,
}

impl DbApi {
    pub fn new(telemetry: telemetry::Telemetry) -> Self {
        Self { telemetry }
    }

    /// Open the global database (tenants, sessions, categories,
    /// `vm_templates`, networks, deployment configs).
    pub async fn open_global(&self, path: &str) -> Result<db::GlobalDb> {
        if path.trim().is_empty() {
            return Err(Error::InvalidInput("path must not be empty".into()));
        }
        db::GlobalDb::open(path)
            .await
            .map(|gdb| gdb.with_telemetry(self.telemetry.clone()))
            .map_err(|e| Error::Database(e.to_string()))
    }

    /// Look up a tenant by id.
    pub async fn tenant_by_id(
        &self,
        gdb: &db::GlobalDb,
        tenant_id: &str,
    ) -> Result<Option<core::Tenant>> {
        let conn = gdb.conn().map_err(|e| Error::Database(e.to_string()))?;
        db::global::tenants::get_by_id(&conn, tenant_id)
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }
}
