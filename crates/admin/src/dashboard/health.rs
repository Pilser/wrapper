//! Health checks for the admin dashboard (live global-DB probe).

use core::Error;

/// Result of a health sweep over wrapper subsystems.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy: bool,
    pub checks: Vec<String>,
}

/// Probe the global database: the connection must open and the `tenants`
/// table must be readable.
pub async fn check_health(db: &db::GlobalDb) -> Result<HealthStatus, Error> {
    let conn = match db.conn() {
        Ok(conn) => conn,
        Err(e) => {
            return Ok(HealthStatus {
                healthy: false,
                checks: vec![format!("admin db: unavailable ({e})")],
            });
        }
    };
    match db::global::tenants::list(&conn).await {
        Ok(tenants) => Ok(HealthStatus {
            healthy: true,
            checks: vec![
                "admin: ok".to_string(),
                format!("tenants: {} row(s) readable", tenants.len()),
            ],
        }),
        Err(e) => Ok(HealthStatus {
            healthy: false,
            checks: vec![format!("admin db: tenants unreadable ({e})")],
        }),
    }
}
