//! Boot source configuration helper.

use super::config::BootSourceConfig;

/// Build a boot source for an initramfs-based VM.
pub fn boot_source(kernel_path: &str, initrd_path: &str) -> BootSourceConfig {
    BootSourceConfig {
        kernel_image_path: kernel_path.to_string(),
        initrd_path: Some(initrd_path.to_string()),
        // Boot args for an initramfs root: no quiet, no disk mounts needed.
        // random.trust_cpu=on seeds the CRNG from RDRAND when the host CPU
        // exposes it (microVMs have no virtio-rng; the guest also runs
        // haveged as a second entropy source).
        boot_args: Some(
            "console=ttyS0 reboot=k panic=1 pci=off init=/init random.trust_cpu=on quiet loglevel=0"
                .to_string(),
        ),
    }
}

/// Build a boot source for an initramfs-based VM, passing the guest IP and
/// gateway on the kernel command line. The guest `/init` reads `vm_ip=` and
/// `gw=` from `/proc/cmdline` so the address the host allocated (IP pool) is
/// the address the guest configures — no more hardcoded-IP mismatch.
///
/// Also passes `guest_seed=` (64 fresh bytes from the host RNG as hex). The
/// guest's `/usr/sbin/seedcrng` credits it into the kernel CRNG via
/// RNDADDENTROPY — without this the microVM never initializes its CRNG (no
/// virtio-rng, almost no interrupt entropy) and dropbear/tokio block on
/// getrandom forever. Same trust model as the host that launched the VM.
///
/// `tenant_id` rides the cmdline too (`tenant_id=`): the shared initramfs is
/// identical for all tenants, so per-boot identity (used for zw link minting
/// and supervisor heartbeat) must be injected, not baked in.
pub fn boot_source_with_net(
    kernel_path: &str,
    initrd_path: &str,
    vm_ip: &str,
    gateway: &str,
    tenant_id: &str,
) -> BootSourceConfig {
    let seed = guest_seed_hex();
    BootSourceConfig {
        kernel_image_path: kernel_path.to_string(),
        initrd_path: Some(initrd_path.to_string()),
        boot_args: Some(format!(
            "console=ttyS0 reboot=k panic=1 pci=off init=/init vm_ip={vm_ip} gw={gateway} random.trust_cpu=on guest_seed={seed} tenant_id={tenant_id} quiet loglevel=0"
        )),
    }
}

/// 64 fresh random bytes from the host as lowercase hex (128 chars).
/// Falls back to empty string when the host RNG is unreadable — the guest
/// then relies on haveged alone (slow) via its /etc/seed.bin fallback.
fn guest_seed_hex() -> String {
    use std::io::Read;
    let mut buf = [0u8; 64];
    let ok = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_ok();
    if !ok {
        return String::new();
    }
    let mut hex = String::with_capacity(128);
    for b in buf {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}
