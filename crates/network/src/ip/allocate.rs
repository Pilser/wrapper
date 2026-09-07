//! Allocate an IP from the pool.

use shared::IpAllocation;
use telemetry::Level;

use super::pool::IpPool;

pub fn allocate_ip(pool: &IpPool, tenant_id: &str) -> Option<IpAllocation> {
    let mut free = pool.free.lock().unwrap();
    let ip = free.iter().next().cloned()?;
    free.remove(&ip);
    pool.taken.lock().unwrap().insert(tenant_id.to_string(), ip.clone());
    let alloc = IpAllocation {
        ip_address: ip.clone(),
        tap_device: format!("tap_{}", tenant_id.chars().take(10).collect::<String>()),
        tenant_id: Some(tenant_id.to_string()),
        allocated_at: shared::now_rfc3339(),
    };
    pool.telemetry().builder().level(Level::Info).kind("network.ip.allocated")
        .category("network").message(format!("allocated {ip} to {tenant_id}"))
        .tenant_id(Some(tenant_id.to_string())).data("ip", ip).emit();
    Some(alloc)
}
