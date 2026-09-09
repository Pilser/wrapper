//! Wake-on-request runtime: ensure a tenant's VM is running.
//!
//! `VmRuntime::ensure_running(tenant_id)` is the single entry point the proxy
//! calls before forwarding a request. It returns the VM's `IP:port` (the
//! zerowrapper port):
//! * recorded VM answers TCP -> return it (hot path);
//! * recorded but dark + snapshot on disk -> RESTORE it (~0.2 s, same IP);
//! * otherwise -> fresh boot (~9 s), snapshot taken later at idle sleep.
//!
//! Snapshots live in `<snapshots_dir>/<tenant_id>/{vmstate,mem}` (host disk,
//! ~512 MB per sleeping tenant = guest RAM size). The tap name and MAC are
//! deterministic per tenant, so a restore reattaches the identical NIC the
//! snapshot was taken with.

use shared::{NetworkConfig, Tenant, TenantStatus, VmTemplate};
use telemetry::Telemetry;

use crate::firecracker::FirecrackerClient;
use crate::firecracker::network::{gateway_for_ip, guest_mac_for_tenant, tap_name_for_tenant};
use crate::lifecycle::boot::NetAttach;
use crate::manager::VmManager;

// Network helpers live in the `network` crate; re-exported here for convenience.
use network::ip::IpPool;
use network::tap::{self, TapCreateOptions};

/// Host bridge all tenant taps attach to. Holds the gateway address.
const BRIDGE_NAME: &str = "br0";

/// The port the zerowrapper listens on inside every VM.
pub const ZERO_WRAPPER_PORT: u16 = 9092;

/// Default memory (MiB) and vCPU used when a template does not specify them.
const DEFAULT_MEM_MIB: u32 = 512;
const DEFAULT_VCPU: u32 = 2;

#[derive(Clone)]
pub struct VmRuntime {
    vm_manager: VmManager,
    ip_pool: IpPool,
    db: db::global::GlobalDb,
    telemetry: Telemetry,
    kernel_path: String,
    initrd_path: String,
    snapshots_dir: String,
}

impl VmRuntime {
    pub fn new(
        firecracker_bin: impl Into<String>,
        db: db::global::GlobalDb,
        net_cfg: NetworkConfig,
        telemetry: Telemetry,
        kernel_path: impl Into<String>,
        initrd_path: impl Into<String>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            vm_manager: VmManager::with_telemetry(firecracker_bin, telemetry.clone()),
            ip_pool: IpPool::with_telemetry(net_cfg, telemetry.clone()),
            db,
            telemetry,
            kernel_path: kernel_path.into(),
            initrd_path: initrd_path.into(),
            snapshots_dir: "data/snapshots".to_string(),
        })
    }

    /// Directory holding per-tenant snapshots (`<dir>/<tenant>/{vmstate,mem}`).
    pub fn with_snapshots_dir(mut self, dir: impl Into<String>) -> Self {
        self.snapshots_dir = dir.into();
        self
    }

    /// Snapshot files for a tenant (state, mem).
    pub fn snapshot_paths(&self, tenant_id: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::path::Path::new(&self.snapshots_dir).join(tenant_id);
        (dir.join("vmstate"), dir.join("mem"))
    }

    /// Delete a tenant's snapshot files (tenant deletion).
    pub fn delete_snapshots(&self, tenant_id: &str) -> anyhow::Result<()> {
        let dir = std::path::Path::new(&self.snapshots_dir).join(tenant_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Ensure the tenant's VM is running; return its `IP:port`.
    pub async fn ensure_running(&self, tenant_id: &str) -> anyhow::Result<String> {
        // 1. Recorded VM that answers: hot path.
        if let Some(addr) = self.live_vm(tenant_id).await? {
            return Ok(addr);
        }
        // 2. Recorded but dark + snapshot on disk: resume in place (same IP).
        if let Some(addr) = self.restore_if_snapshotted(tenant_id).await? {
            return Ok(addr);
        }
        // 3. Fresh boot (clears any stale row first).
        self.fresh_boot(tenant_id).await
    }

    /// Snapshot the tenant's live VM to disk. Returns true when a usable
    /// snapshot now exists (idle path calls this before killing the VM and
    /// then KEEPS the row so wake restores instead of rebooting).
    pub async fn snapshot_tenant(&self, tenant_id: &str) -> anyhow::Result<bool> {
        let conn = self.db.conn()?;
        let rows = db::global::networks::all(&conn).await?;
        if rows.iter().all(|r| r.tenant_id != tenant_id) {
            return Ok(false);
        }
        let sock = sock_path(tenant_id);
        if !sock.exists() {
            return Ok(false);
        }
        let (state_path, mem_path) = self.snapshot_paths(tenant_id);
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let client = FirecrackerClient::new(sock.to_string_lossy().to_string());
        let ok = crate::lifecycle::snapshot::snapshot_vm(
            &self.telemetry,
            &client,
            tenant_id,
            &state_path.to_string_lossy(),
            &mem_path.to_string_lossy(),
        )
        .await
        .is_ok();
        Ok(ok)
    }

    /// Recorded address that answers TCP on the zw port.
    async fn live_vm(&self, tenant_id: &str) -> anyhow::Result<Option<String>> {
        let conn = self.db.conn()?;
        let rows = db::global::networks::all(&conn).await?;
        let Some(res) = rows.into_iter().find(|r| r.tenant_id == tenant_id) else {
            return Ok(None);
        };
        let addr = format!("{ip}:{port}", ip = res.ip_address, port = ZERO_WRAPPER_PORT);
        if probe_tcp(&addr).await {
            Ok(Some(addr))
        } else {
            Ok(None)
        }
    }

    /// Resume a dark recorded VM from its on-disk snapshot (same IP + tap).
    /// Returns None when there is no row, no snapshot files, or the restore
    /// fails (caller falls back to fresh boot).
    async fn restore_if_snapshotted(
        &self,
        tenant_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let conn = self.db.conn()?;
        let rows = db::global::networks::all(&conn).await?;
        let Some(res) = rows.into_iter().find(|r| r.tenant_id == tenant_id) else {
            return Ok(None);
        };
        let (state_path, mem_path) = self.snapshot_paths(tenant_id);
        if !state_path.exists() || !mem_path.exists() {
            // Stale row, nothing to restore from: drop it for fresh boot.
            let _ = db::global::networks::release_all(&conn, tenant_id).await;
            let _ = self.ip_pool.release(tenant_id).await;
            return Ok(None);
        }
        // Recreate the identical host NIC, spawn, load + resume.
        let tap = tap_name_for_tenant(tenant_id);
        let gateway = gateway_for_ip(&res.ip_address);
        if network::bridge::create::ensure_bridge(BRIDGE_NAME, &gateway).is_err() {
            return Ok(None);
        }
        if tap::create_tap(TapCreateOptions {
            name: tap.clone(),
            ip: res.ip_address.clone(),
        })
        .is_err()
        {
            return Ok(None);
        }
        let _ = tap::bring_tap_up(&tap);
        let sock = sock_path(tenant_id);
        let client = match FirecrackerClient::spawn_firecracker(
            &self.vm_manager.firecracker_bin,
            &sock,
        )
        .await
        {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let ok = crate::lifecycle::restore::restore_vm(
            &self.telemetry,
            &client,
            tenant_id,
            &state_path.to_string_lossy(),
            &mem_path.to_string_lossy(),
            true,
        )
        .await
        .is_ok();
        if !ok {
            return Ok(None);
        }
        let _ = db::global::tenants::set_status(&conn, tenant_id, TenantStatus::Ready.as_str()).await;
        let addr = format!("{ip}:{ZERO_WRAPPER_PORT}", ip = res.ip_address);
        self.telemetry
            .builder()
            .level(telemetry::Level::Info)
            .kind("vm.restore.wake")
            .category("vm")
            .tenant_id(Some(tenant_id.to_string()))
            .message(format!("VM resumed for tenant {tenant_id} at {addr}"))
            .emit();
        Ok(Some(addr))
    }

    /// Full fresh boot for a tenant with no usable VM or snapshot.
    async fn fresh_boot(&self, tenant_id: &str) -> anyhow::Result<String> {
        // Look up the tenant to find their category/template.
        let tenant = self.tenant(tenant_id).await?;
        let template = self.template_for_tenant(&tenant).await?;

        // Drop any stale row (dark VM, no snapshot) so the new assignment
        // is the only record.
        if let Ok(conn) = self.db.conn() {
            let _ = db::global::networks::release_all(&conn, tenant_id).await;
        }
        let _ = self.ip_pool.release(tenant_id).await;

        // 3. Assign an IP from the pool.
        let ip = self
            .ip_pool
            .allocate(tenant_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("IP pool exhausted"))?;

        // 4. Host networking (needs root): bridge with gateway address,
        // tenant tap attached to the bridge, tap up, forwarding/NAT for
        // guest egress. NAT is best-effort: without it the guest is still
        // reachable from the host (SSH :2222, zw :9092, status :8080).
        //
        // The tap itself gets NO IP address: it is a pure L2 bridge member.
        // Assigning the guest IP to the tap makes the HOST claim that IP
        // (local route) so host TCP connects hit the host stack (RST) and
        // ping "works" without ever reaching the guest. The guest owns its
        // IP (via vm_ip= cmdline); the host reaches it through br0.
        let gateway = gateway_for_ip(&ip);
        let tap = tap_name_for_tenant(tenant_id);
        let guest_mac = guest_mac_for_tenant(tenant_id);
        network::bridge::create::ensure_bridge(BRIDGE_NAME, &gateway)?;
        tap::create_tap(TapCreateOptions {
            name: tap.clone(),
            ip: ip.clone(),
        })?;
        tap::bring_tap_up(&tap)?;
        Self::enable_forwarding_best_effort(&self.telemetry, &ip);

        // 5. Boot Firecracker for this tenant.
        let sock = sock_path(tenant_id);
        let client =
            FirecrackerClient::spawn_firecracker(&self.vm_manager.firecracker_bin, &sock).await?;

        // Build a concrete template with the host-side kernel + initrd paths.
        let boot_template = VmTemplate {
            id: template.id,
            name: template.name,
            description: template.description,
            rootfs_path: String::new(),
            kernel_path: self.kernel_path.clone(),
            initrd_path: self.initrd_path.clone(),
            vcpu_count: if template.vcpu_count == 0 {
                DEFAULT_VCPU
            } else {
                template.vcpu_count
            },
            memory_mb: if template.memory_mb == 0 {
                DEFAULT_MEM_MIB
            } else {
                template.memory_mb
            },
            requires_gpu: template.requires_gpu,
            status: template.status,
            created_at: template.created_at,
            updated_at: template.updated_at,
        };
        crate::lifecycle::boot::boot_vm_with_net(
            &self.telemetry,
            &client,
            tenant_id,
            &boot_template,
            Some(&NetAttach::new(&tap, &guest_mac, &ip)),
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // 6. Record the network assignment + VM process in the DB.
        let conn = self.db.conn()?;
        db::global::networks::reserve(&conn, tenant_id, &ip, &tap).await?;
        db::global::tenants::set_vm_address(&conn, tenant_id, &ip).await?;
        // Mark Ready (a reaped tenant was Asleep; a fresh one Active).
        let _ = db::global::tenants::set_status(&conn, tenant_id, TenantStatus::Ready.as_str()).await;

        let addr = format!("{ip}:{ZERO_WRAPPER_PORT}");
        self.telemetry
            .builder()
            .level(telemetry::Level::Info)
            .kind("vm.boot.completed")
            .category("vm")
            .tenant_id(Some(tenant_id.to_string()))
            .message(format!("VM awake for tenant {tenant_id} at {addr}"))
            .data("ip", ip)
            .emit();
        Ok(addr)
    }

    /// Pre-mark IPs already reserved in the DB (restart recovery): a fresh
    /// in-memory pool would otherwise hand a live VM's address to a new
    /// tenant. Call once at startup before serving.
    pub async fn premark_reserved(&self) -> anyhow::Result<usize> {
        let conn = self.db.conn()?;
        let rows = db::global::networks::all(&conn).await?;
        let n = rows.len();
        for row in rows {
            self.ip_pool.mark_taken(&row.tenant_id, &row.ip_address);
        }
        Ok(n)
    }

    /// Fetch the tenant row.
    async fn tenant(&self, tenant_id: &str) -> anyhow::Result<Tenant> {
        let conn = self.db.conn()?;
        db::global::tenants::get_by_id(&conn, tenant_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("tenant {tenant_id} not found"))
    }

    /// Resolve the tenant's category → VM template.
    async fn template_for_tenant(&self, tenant: &Tenant) -> anyhow::Result<VmTemplate> {
        let conn = self.db.conn()?;
        let category = db::global::categories::get_by_id(&conn, &tenant.category_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("category {} not found", tenant.category_id))?;
        db::global::vm_templates::get_by_id(&conn, &category.vm_template_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("vm template {} not found", category.vm_template_id))
    }

    /// Best-effort guest egress: enable IPv4 forwarding and masquerade the    /// tenant /24. Failures only warn (host→guest still works without NAT);
    /// needs root + iptables.
    fn enable_forwarding_best_effort(telemetry: &Telemetry, vm_ip: &str) {
        let cidr = format!("{}/24", gateway_for_ip(vm_ip));
        let steps: &[(&str, &[&str])] = &[
            ("sysctl", &["-w", "net.ipv4.ip_forward=1"]),
            (
                "iptables",
                &[
                    "-t", "nat", "-A", "POSTROUTING", "-s", &cidr, "-j", "MASQUERADE",
                ],
            ),
            ("iptables", &["-A", "FORWARD", "-s", &cidr, "-j", "ACCEPT"]),
        ];
        // NOTE: `cidr` is borrowed by `steps`; keep it alive for the loop.
        for (bin, args) in steps {
            let ok = std::process::Command::new(bin)
                .args(*args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !ok {
                telemetry
                    .builder()
                    .level(telemetry::Level::Warn)
                    .kind("vm.net.nat_skipped")
                    .category("vm")
                    .message(format!("best-effort net step failed: {bin} {args:?}"))
                    .emit();
            }
        }
    }
}

/// Deterministic API socket per tenant (spawn, snapshot, reaper agree).
fn sock_path(tenant_id: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("/tmp/fc_{tenant_id}.sock"))
}

/// Fast TCP probe (800ms) — true when something accepts on `IP:port`.
async fn probe_tcp(addr: &str) -> bool {
    let fut = tokio::net::TcpStream::connect(addr);
    tokio::time::timeout(std::time::Duration::from_millis(800), fut)
        .await
        .is_ok_and(|r| r.is_ok())
}
