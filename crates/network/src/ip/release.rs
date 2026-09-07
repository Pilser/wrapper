//! Release an IP back to the pool.

use super::pool::IpPool;

pub fn release_ip(pool: &IpPool, tenant_id: &str) -> Option<String> {
    let ip = pool.taken.lock().unwrap().remove(tenant_id)?;
    pool.free.lock().unwrap().insert(ip.clone());
    Some(ip)
}
