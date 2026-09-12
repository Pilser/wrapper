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

/// Default per-tenant data-disk size (GiB, sparse — host pays only written
/// blocks). Raised per tenant later via the admin resize action.
pub const DEFAULT_DATA_DISK_GB: u64 = 2;

/// Hard cap for data-disk resizes (GiB). Grow-only; shrinking is refused.
pub const MAX_DATA_DISK_GB: u64 = 64;

#[derive(Clone)]
pub struct VmRuntime {
    vm_manager: VmManager,
    ip_pool: IpPool,
    db: db::global::GlobalDb,
    telemetry: Telemetry,
    kernel_path: String,
    initrd_path: String,
    snapshots_dir: String,
    data_disks_dir: String,
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
            data_disks_dir: "data/disks".to_string(),
        })
    }

    /// Directory holding per-tenant snapshots (`<dir>/<tenant>/{vmstate,mem}`).
    pub fn with_snapshots_dir(mut self, dir: impl Into<String>) -> Self {
        self.snapshots_dir = dir.into();
        self
    }

    /// Directory holding per-tenant data-disk images (`<dir>/<tenant>.img`).
    pub fn with_data_disks_dir(mut self, dir: impl Into<String>) -> Self {
        self.data_disks_dir = dir.into();
        self
    }

    /// Host path of a tenant's data-disk image. The tenant id is validated
    /// to a safe charset so it can never escape the disks directory.
    pub fn data_disk_path(&self, tenant_id: &str) -> anyhow::Result<std::path::PathBuf> {
        if tenant_id.is_empty()
            || !tenant_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!("invalid tenant id for disk path");
        }
        Ok(std::path::Path::new(&self.data_disks_dir).join(format!("{tenant_id}.img")))
    }

    /// Ensure the tenant's data disk exists (sparse `fallocate` + `mkfs.ext4`,
    /// ~0.1 s for 2 GiB, paid once per tenant). Returns the image path.
    /// Existing images are returned untouched — this never reformats.
    pub async fn ensure_data_disk(&self, tenant_id: &str) -> anyhow::Result<std::path::PathBuf> {
        let path = self.data_disk_path(tenant_id)?;
        if path.exists() {
            return Ok(path);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let fallocate = tokio::process::Command::new("fallocate")
            .args(["-l", &format!("{}G", DEFAULT_DATA_DISK_GB)])
            .arg(&path)
            .status()
            .await?;
        if !fallocate.success() {
            anyhow::bail!("fallocate data disk failed");
        }
        let mkfs = tokio::process::Command::new("mkfs.ext4")
            .args(["-q", "-E", "lazy_itable_init,lazy_journal_init", "-L", "zwdata"])
            .arg(&path)
            .status()
            .await?;
        if !mkfs.success() {
            let _ = std::fs::remove_file(&path);
            anyhow::bail!("mkfs.ext4 data disk failed");
        }
        self.telemetry
            .builder()
            .level(telemetry::Level::Info)
            .kind("vm.disk.created")
            .category("vm")
            .tenant_id(Some(tenant_id.to_string()))
            .data("path", path.to_string_lossy().to_string())
            .data("size_gb", DEFAULT_DATA_DISK_GB.to_string())
            .message(format!("data disk created for tenant {tenant_id}"))
            .emit();
        Ok(path)
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

    /// Apparent size (bytes) of a tenant's data-disk image. None when the
    /// image does not exist (pure `stat`, no fork — safe on hot paths).
    pub fn disk_size_bytes(&self, tenant_id: &str) -> Option<u64> {
        let path = self.data_disk_path(tenant_id).ok()?;
        std::fs::metadata(&path).ok().map(|m| m.len())
    }

    /// Real host blocks consumed by the image (sparse-aware: a 2 GiB image
    /// holding 200 MiB reports ~200 MiB). Best-effort: None when `du` is
    /// missing or fails. Keep off hot paths (forks a process).
    pub async fn disk_used_bytes(&self, tenant_id: &str) -> Option<u64> {
        let path = self.data_disk_path(tenant_id).ok()?;
        if !path.exists() {
            return None;
        }
        let out = tokio::process::Command::new("du")
            .args(["-B1"])
            .arg(&path)
            .output()
            .await
            .ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    }

    /// Filesystem size (bytes) of an ext4 image via `dumpe2fs -h`
    /// (Block count x Block size). None when unreadable — callers fall back
    /// to the file size.
    async fn fs_size_bytes(path: &std::path::Path) -> Option<u64> {
        let out = tokio::process::Command::new("dumpe2fs")
            .args(["-h"])
            .arg(path)
            .output()
            .await
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut blocks: Option<u64> = None;
        let mut bsize: Option<u64> = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("Block count:") {
                blocks = v.trim().parse().ok();
            } else if let Some(v) = line.strip_prefix("Block size:") {
                bsize = v.trim().parse().ok();
            }
        }
        Some(blocks? * bsize?)
    }

    /// Flush the guest's page cache over SSH (`sync`) so a subsequent kill
    /// lands on a quiescent filesystem. Best-effort with a short timeout:
    /// failure falls back to kill-only (journal replay), never blocks.
    pub async fn sync_guest_disk(&self, tenant_id: &str) {
        let ip: Option<String> = match self.db.conn() {
            Ok(c) => db::global::networks::all(&c)
                .await
                .ok()
                .and_then(|rows| {
                    rows.into_iter()
                        .find(|r| r.tenant_id == tenant_id)
                        .map(|r| r.ip_address)
                }),
            Err(_) => None,
        };
        let Some(ip) = ip else { return };
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            tokio::process::Command::new("ssh")
                .args([
                    "-p",
                    "2222",
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "StrictHostKeyChecking=no",
                    "-o",
                    "ConnectTimeout=5",
                    &format!("root@{ip}"),
                    "sync",
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status(),
        )
        .await;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    /// Kill a tenant's Firecracker process and remove its (now stale) API
    /// socket file, so an offline disk operation can proceed and the next
    /// spawn binds cleanly. Guest RAM state is discarded; disk data is
    /// untouched (page cache synced first, ext4 journal replays on mount).
    pub async fn kill_vm_process(&self, tenant_id: &str) {
        self.sync_guest_disk(tenant_id).await;
        let pattern = format!("/tmp/fc_{tenant_id}.sock");
        kill_process_pattern(&pattern).await;
        let _ = std::fs::remove_file(format!("/tmp/fc_{tenant_id}.sock"));
    }

    /// Grow a tenant's data disk to `size_gb` GiB without touching existing
    /// data: `truncate` only appends zero blocks, then `resize2fs` grows the
    /// filesystem into them. Grow-only (`size_gb` must exceed current size,
    /// capped at `MAX_DATA_DISK_GB`).
    ///
    /// SAFETY: the VM must be DOWN (no FC socket). A live guest holds cached
    /// filesystem metadata; growing under it corrupts. The admin handler
    /// kills the VM first. Sleep snapshots are deleted: they describe the
    /// old geometry and must never restore over the resized disk (the next
    /// wake is a fresh boot — disk data intact, RAM state not).
    pub async fn resize_data_disk(
        &self,
        tenant_id: &str,
        size_gb: u64,
    ) -> anyhow::Result<u64> {
        if size_gb == 0 || size_gb > MAX_DATA_DISK_GB {
            anyhow::bail!("size_gb must be 1..={MAX_DATA_DISK_GB}");
        }
        let path = self.data_disk_path(tenant_id)?;
        if !path.exists() {
            anyhow::bail!("no data disk for tenant (never booted with disk support?)");
        }
        if fc_socket_path(tenant_id).exists() {
            anyhow::bail!("vm must be shut down before resize (socket present)");
        }
        let cur_file = std::fs::metadata(&path)?.len();
        // Filesystem size (blocks) can LAG the file size when an earlier
        // attempt grew the file but failed before resize2fs: compare the
        // request against the fs, and skip truncate when the file is
        // already big enough (makes retry repair instead of refuse).
        let cur_fs = Self::fs_size_bytes(&path).await.unwrap_or(cur_file);
        let want = size_gb * 1024 * 1024 * 1024;
        if want <= cur_fs {
            anyhow::bail!("grow-only: {size_gb}GiB is not larger than current filesystem");
        }
        // 1. Grow the file when needed (append-only; existing bytes
        //    untouched). Skipped when a previous attempt already grew it.
        if want > cur_file {
            let st = tokio::process::Command::new("truncate")
                .args(["-s", &format!("{want}")])
                .arg(&path)
                .status()
                .await?;
            if !st.success() {
                anyhow::bail!("truncate data disk failed");
            }
        }
        // 2. Filesystem check (replays the journal from the kill; must be
        //    clean before resize). e2fsck -p exits 0 (clean) or 1 (errors
        //    corrected — equally fine); 2+ means uncorrected trouble.
        //    Refusal here leaves a valid fs at the OLD size: data safe,
        //    resize aborted.
        let ck = tokio::process::Command::new("e2fsck")
            .args(["-p", "-f"])
            .arg(&path)
            .status()
            .await?;
        match ck.code() {
            Some(0) | Some(1) => {}
            other => {
                anyhow::bail!("e2fsck data disk failed (exit {other:?}; image left at old size, data intact)")
            }
        }
        // 3. Grow the filesystem into the new space.
        let rs = tokio::process::Command::new("resize2fs")
            .arg(&path)
            .status()
            .await?;
        if !rs.success() {
            anyhow::bail!("resize2fs failed (file grown, fs at old size — retry or investigate)");
        }
        // 4. Old-geometry snapshots must never restore over the new disk.
        let _ = self.delete_snapshots(tenant_id);
        let now = std::fs::metadata(&path)?.len();
        self.telemetry
            .builder()
            .level(telemetry::Level::Info)
            .kind("vm.disk.resized")
            .category("vm")
            .tenant_id(Some(tenant_id.to_string()))
            .data("size_bytes", now.to_string())
            .message(format!("data disk resized to {size_gb}GiB for tenant {tenant_id}"))
            .emit();
        Ok(now)
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
        kill_stale_holder(tenant_id).await;
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
            // Preserve evidence: the fresh-boot fallback below truncates the
            // per-VM FC log (same deterministic path), destroying the load
            // error. Keep a copy next to the socket.
            let failed_log = sock.with_extension("restore-failed.log");
            let _ = std::fs::copy(sock.with_extension("log"), &failed_log);
            self.telemetry
                .builder()
                .level(telemetry::Level::Warn)
                .kind("vm.restore.fallback")
                .category("vm")
                .tenant_id(Some(tenant_id.to_string()))
                .message(format!(
                    "restore failed, falling back to fresh boot (see {})",
                    failed_log.display()
                ))
                .emit();
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
        // is the only record. Also delete stale snapshots: they describe the
        // dead VM (old IP/tenant state in RAM) and must never restore over
        // this fresh boot.
        if let Ok(conn) = self.db.conn() {
            let _ = db::global::networks::release_all(&conn, tenant_id).await;
        }
        let _ = self.ip_pool.release(tenant_id).await;
        let _ = self.delete_snapshots(tenant_id);

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

        // 5. Boot Firecracker for this tenant (clearing any orphan holder
        // first so the tap attach cannot EBUSY).
        kill_stale_holder(tenant_id).await;
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
        // Tenant data disk: created once (~0.1 s), attached every boot.
        // Best-effort at the ensure step only — a tenant must still boot
        // (diskless, as before) if disk creation fails; the failure is
        // loud in telemetry so it cannot go unnoticed.
        let data_disk = match self.ensure_data_disk(tenant_id).await {
            Ok(p) => Some(p),
            Err(e) => {
                self.telemetry
                    .builder()
                    .level(telemetry::Level::Warn)
                    .kind("vm.disk.ensure_failed")
                    .category("vm")
                    .tenant_id(Some(tenant_id.to_string()))
                    .message(format!("data disk unavailable, booting diskless: {e}"))
                    .emit();
                None
            }
        };
        crate::lifecycle::boot::boot_vm_with_net(
            &self.telemetry,
            &client,
            tenant_id,
            &boot_template,
            Some(&NetAttach::new(&tap, &guest_mac, &ip)),
            data_disk.as_deref(),
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
pub fn fc_socket_path(tenant_id: &str) -> std::path::PathBuf {
    sock_path(tenant_id)
}

/// Kill every process matching `pattern` (full-cmdline match), verified.
/// SIGTERM first (lets Firecracker flush + exit cleanly), up to 5s grace,
/// then SIGKILL stragglers and confirm absence. Fire-and-hope pkill caused
/// a real incident: a SIGTERMed FC still answering during the next wake
/// made the old generation serve traffic the new boot was meant to take.
pub async fn kill_process_pattern(pattern: &str) {
    let quiet = || {
        let mut c = tokio::process::Command::new("pkill");
        c.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        c
    };
    let alive = || async {
        tokio::process::Command::new("pgrep")
            .args(["-f", pattern])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    };
    let _ = quiet().args(["-f", pattern]).status().await;
    for _ in 0..10 {
        if !alive().await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    let _ = quiet().args(["-9", "-f", pattern]).status().await;
    for _ in 0..4 {
        if !alive().await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Kill a stale Firecracker still holding this tenant's tap/socket.
/// Wrapper restarts orphan VMs BY DESIGN (guests outlive the wrapper so a
/// restart doesn't kill tenants). But a dead-yet-holding FC (guest app
/// crashed, process alive) blocks the tap with EBUSY: the next boot's PUT
/// /network-interfaces then 400s ("Resource busy"). Clearing the holder
/// before every spawn makes boots idempotent across generations.
/// Safe: the wrapper's own cmdline never contains the socket pattern.
async fn kill_stale_holder(tenant_id: &str) {
    let pattern = format!("/tmp/fc_{tenant_id}.sock");
    kill_process_pattern(&pattern).await;
}

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
