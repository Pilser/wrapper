//! Detect IP conflicts.

use super::pool::IpPool;

pub fn is_conflict(pool: &IpPool, ip: &str) -> bool {
    // An IP that is neither free nor assigned to a tenant is ambiguous; a
    // re-request of an already-taken address is always a conflict.
    pool.taken.lock().unwrap().values().any(|v| v == ip)
}

pub fn detect_conflicts(pool: &IpPool) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    pool.taken
        .lock()
        .unwrap()
        .values()
        .filter(|v| !seen.insert((*v).clone()))
        .cloned()
        .collect()
}
