//! Firecracker network-interface configuration (`PUT /network-interfaces/{id}`).
//!
//! Without this call the guest has no virtual NIC: the tap device the host
//! creates is never attached to the VM, `eth0` does not exist inside the
//! guest, and SSH/the zerowrapper port are unreachable. Every boot must
//! attach exactly one interface backed by the tenant's tap device.

use serde::{Deserialize, Serialize};

/// A virtual network interface attached to a microVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceConfig {
    pub iface_id: String,
    pub guest_mac: String,
    pub host_dev_name: String,
}

/// Build the interface config for a tenant VM: `eth0` on the host side
/// backed by `tap_name`, with a deterministic locally-administered MAC
/// derived from the tenant id so reboots keep the same address.
pub fn network_interface(tap_name: &str, guest_mac: &str) -> NetworkInterfaceConfig {
    NetworkInterfaceConfig {
        iface_id: "eth0".to_string(),
        guest_mac: guest_mac.to_string(),
        host_dev_name: tap_name.to_string(),
    }
}

/// Deterministic unicast locally-administered MAC (`02:xx:xx:xx:xx:xx`)
/// derived from the tenant id. Stable across reboots, unique per tenant.
pub fn guest_mac_for_tenant(tenant_id: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in tenant_id.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    // Use 5 bytes of the hash; top byte fixed to 0x02 (locally administered).
    let bytes = hash.to_be_bytes();
    format!(
        "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
    )
}

/// Derive the gateway address from a guest IP by replacing the last octet
/// with `.1` (e.g. `172.16.0.5` → `172.16.0.1`). Falls back to `172.16.0.1`
/// when the IP does not parse as IPv4.
pub fn gateway_for_ip(vm_ip: &str) -> String {
    let mut parts: Vec<&str> = vm_ip.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        parts[3] = "1";
        parts.join(".")
    } else {
        "172.16.0.1".to_string()
    }
}

/// Sanitize a tenant id into a valid Linux interface name (≤15 chars,
/// alphanumeric only): `tap_` + first 11 alphanumerics of the tenant id.
pub fn tap_name_for_tenant(tenant_id: &str) -> String {
    let clean: String = tenant_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(11)
        .collect();
    format!("tap_{clean}")
}
