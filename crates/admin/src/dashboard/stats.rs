//! Dashboard statistics (live aggregates over the global database).

use shared::{Error, Result};

/// Fleet-wide counters shown on the admin dashboard.
#[derive(Debug, Clone, Default)]
pub struct SystemStats {
    pub total_tenants: u64,
    pub active_vms: u64,
    pub total_categories: u64,
}

/// Collect a snapshot of fleet statistics from the global tables: tenant
/// count from `tenants`, category count from `categories`, and active VMs as
/// tenants with an assigned `vm_ip`.
pub async fn collect_stats(db: &db::GlobalDb) -> Result<SystemStats> {
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let tenants = db::global::tenants::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let categories = db::global::categories::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    Ok(SystemStats {
        total_tenants: tenants.len() as u64,
        active_vms: tenants.iter().filter(|t| t.vm_ip.is_some()).count() as u64,
        total_categories: categories.len() as u64,
    })
}
