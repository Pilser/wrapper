//! Wake-on-request runtime: ensure a tenant's container is running.
//!
//! `VmRuntime::ensure_running(tenant_id)` is the single entry point the proxy
//! calls before forwarding a request. It returns the tenant's `IP:port` (the
//! zerowrapper port):
//! * recorded container answers TCP -> return it (hot path);
//! * recorded but paused -> UNPAUSE it (instant, RAM kept);
//! * otherwise -> create + start (~0.5 s).
//!
//! One container per tenant (`zw-<tenant-id>`) on the tenant bridge, stable
//! IP for life, tenant data in a host folder bind-mounted at `/data`. Sleep
//! = `pause`, Stop = `stop`, Kill = `rm` + recreate. No KVM anywhere.

use shared::{NetworkConfig, Tenant, TenantStatus, VmTemplate};
use telemetry::Telemetry;

use crate::docker::{ContainerStats, DockerDriver};

// Network helpers live in the `network` crate; re-exported here for convenience.
use network::ip::IpPool;

/// The port the zerowrapper listens on inside every tenant container.
pub const ZERO_WRAPPER_PORT: u16 = 9092;

/// Default memory (MiB) and vCPU used when a template does not specify them.
const DEFAULT_MEM_MIB: u32 = 512;
const DEFAULT_VCPU: u32 = 2;

/// Host path of the default agent system prompt without a runtime handle
/// (admin handlers in degraded mode). `configured` wins (wrapper `[vm]`
/// `system_prompt_host_path`, relative to the wrapper cwd); otherwise probe
/// `zerowrapper-mono/SYSTEM.md` next to the deployment. Never absolute:
/// returns empty when nothing resolves.
pub fn host_system_prompt_path_for(
    configured: &str,
    tenant_data_root: &str,
) -> std::path::PathBuf {
    if !configured.trim().is_empty() {
        return std::path::PathBuf::from(configured);
    }
    if let Ok(canon) = std::path::Path::new(tenant_data_root).canonicalize() {
        if let Some(deployment) = canon.parent().and_then(|a| a.parent()) {
            let p = deployment.join("zerowrapper-mono/SYSTEM.md");
            if p.exists() {
                return p;
            }
        }
    }
    std::path::PathBuf::new()
}



/// Recursive byte size of a host folder (tenant data usage).
fn dir_size_bytes(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(m) = e.metadata() {
                total = total.saturating_add(m.len());
            }
        }
    }
    total
}

/// Resolve the effective agent system prompt for a tenant:
/// tenant row > category row > global row > host SYSTEM.md file.
/// Returns `(content, source)` where source is `tenant:<id>`,
/// `category:<id>`, `global` or `host-file`.
pub async fn resolve_system_prompt(
    gdb: &db::global::GlobalDb,
    host_path: &std::path::Path,
    tenant_id: &str,
) -> (String, String) {
    if let Ok(conn) = gdb.conn() {
        let category_id = db::global::tenants::get_by_id(&conn, tenant_id)
            .await
            .ok()
            .flatten()
            .map(|t| t.category_id)
            .unwrap_or_default();
        if let Ok((Some(content), source)) =
            db::global::system_prompts::effective(&conn, tenant_id, &category_id).await
        {
            return (content, source);
        }
    }
    let content = std::fs::read_to_string(host_path).unwrap_or_default();
    (content, "host-file".to_string())
}

#[derive(Clone)]
pub struct VmRuntime {
    docker: DockerDriver,
    ip_pool: IpPool,
    db: db::global::GlobalDb,
    telemetry: Telemetry,
    /// Docker bridge network tenants attach to (stable IPs for life).
    network: String,
    subnet: String,
    gateway: String,
    /// Host root holding one folder per tenant, bind-mounted at `/data`.
    tenant_data_root: String,
    /// Configured host path of the default agent system prompt (`[vm]`
    /// `system_prompt_host_path`). Empty = probe relative to the data root.
    system_prompt_host_path: String,
    /// Per-tenant boot mutex: concurrent `ensure_running` for the same tenant
    /// must serialize (otherwise two fresh_boots race on `networks.id` and the
    /// second hits `UNIQUE constraint failed`). A refresh fires 6 parallel
    /// proxy requests — without this they all try to boot at once.
    boot_locks: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>>,
}

impl VmRuntime {
    pub fn new(
        docker: DockerDriver,
        db: db::global::GlobalDb,
        net_cfg: NetworkConfig,
        telemetry: Telemetry,
        network: impl Into<String>,
        subnet: impl Into<String>,
        gateway: impl Into<String>,
        tenant_data_root: impl Into<String>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            docker,
            ip_pool: IpPool::with_telemetry(net_cfg, telemetry.clone()),
            db,
            telemetry,
            network: network.into(),
            subnet: subnet.into(),
            gateway: gateway.into(),
            tenant_data_root: tenant_data_root.into(),
            system_prompt_host_path: String::new(),
            boot_locks: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        })
    }

    /// Host root holding one folder per tenant (bind-mounted at `/data`).
    pub fn with_tenant_data_root(mut self, dir: impl Into<String>) -> Self {
        self.tenant_data_root = dir.into();
        self
    }

    /// Host path of the default agent system prompt (`[vm]`
    /// `system_prompt_host_path`).
    pub fn with_system_prompt_host_path(mut self, path: impl Into<String>) -> Self {
        self.system_prompt_host_path = path.into();
        self
    }

    /// Host folder for a tenant's persistent data. The tenant id is validated
    /// to a safe charset so it can never escape the data root.
    pub fn data_disk_path(&self, tenant_id: &str) -> anyhow::Result<std::path::PathBuf> {
        if tenant_id.is_empty()
            || !tenant_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            anyhow::bail!("invalid tenant id for data path");
        }
        Ok(std::path::Path::new(&self.tenant_data_root).join(tenant_id))
    }

    /// Ensure the tenant's data folder exists (plain `mkdir`, instant; the
    /// folder bind-mounts at `/data` in the container). Returns the path.
    /// Existing folders are returned untouched.
    pub async fn ensure_data_disk(&self, tenant_id: &str) -> anyhow::Result<std::path::PathBuf> {
        let path = self.data_disk_path(tenant_id)?;
        let fresh = !path.exists();
        std::fs::create_dir_all(&path)?;
        // Seed a fresh folder with the agent system prompt (SYSTEM.md
        // resolved tenant > category > global > host file). Best-effort: a
        // seed failure never fails creation — the guest falls back to the
        // baked /etc/supervisor/SYSTEM.md.
        if fresh {
            if let Err(e) = self.seed_prompt_into(&path, tenant_id).await {
                self.telemetry
                    .builder()
                    .level(telemetry::Level::Warn)
                    .kind("vm.disk.seed_failed")
                    .category("vm")
                    .tenant_id(Some(tenant_id.to_string()))
                    .message(format!("prompt seed failed (folder valid): {e}"))
                    .emit();
            }
        }
        Ok(path)
    }

    /// Write the resolved SYSTEM.md into an existing tenant data folder.
    async fn seed_prompt_into(
        &self,
        tenant_dir: &std::path::Path,
        tenant_id: &str,
    ) -> anyhow::Result<()> {
        let (prompt, source) = resolve_system_prompt(
            &self.db,
            &self.host_system_prompt_path(),
            tenant_id,
        )
        .await;
        if prompt.is_empty() {
            anyhow::bail!("no prompt content resolved");
        }
        // Kept OUTSIDE zerowrapper/workspace: the agent must not see or
        // edit its own system prompt; zw reads it via ZS_SYSTEM_PROMPT_FILE.
        let zws = tenant_dir.join("zerowrapper");
        std::fs::create_dir_all(&zws)?;
        std::fs::write(zws.join("SYSTEM.md"), &prompt)?;
        self.telemetry
            .builder()
            .level(telemetry::Level::Info)
            .kind("vm.disk.prompt_seeded")
            .category("vm")
            .data("source", source.clone())
            .message(format!("seeded SYSTEM.md ({source}) into {}", tenant_dir.display()))
            .emit();
        Ok(())
    }

    /// Host path of the default agent system prompt
    /// (`deployment/zerowrapper-mono/SYSTEM.md`).
    pub fn host_system_prompt_path(&self) -> std::path::PathBuf {
        host_system_prompt_path_for(&self.system_prompt_host_path, &self.tenant_data_root)
    }

    /// Remove a tenant's container AND data folder (tenant deletion only).
    /// Kill keeps the folder; only full tenant deletion wipes it.
    pub fn remove_tenant(&self, tenant_id: &str) -> anyhow::Result<()> {
        let _ = self.docker.destroy(tenant_id);
        if let Ok(path) = self.data_disk_path(tenant_id) {
            if path.exists() {
                std::fs::remove_dir_all(&path)?;
            }
        }
        Ok(())
    }

    /// Destroy the tenant container, keeping the data folder (Kill: next
    /// wake recreates fresh). Best-effort and idempotent.
    pub async fn destroy_container(&self, tenant_id: &str) {
        self.docker.destroy(tenant_id).await;
    }

    /// Stop the tenant container, keeping container + data (Stop).
    pub async fn stop_container(&self, tenant_id: &str) {
        self.docker.stop(tenant_id).await;
    }

    /// Container liveness for admin flows: true when Running (pauses count
    /// as present but not live — wake unpauses them).
    pub async fn is_container_live(&self, tenant_id: &str) -> bool {
        self.docker.state(tenant_id).await.0
    }

    /// Restart a live container (stop + start) so it picks up new files
    /// (prompt, binaries). No-op when no container exists.
    pub async fn restart_container(&self, tenant_id: &str) {
        let name = DockerDriver::container_name(tenant_id);
        let (running, _paused) = self.docker.state(tenant_id).await;
        if !running {
            let _ = self.ensure_running(tenant_id).await;
            return;
        }
        self.docker.stop(tenant_id).await;
        if let Err(e) = self.docker.start(tenant_id).await {
            self.telemetry
                .builder()
                .level(telemetry::Level::Warn)
                .kind("vm.restart.failed")
                .category("vm")
                .tenant_id(Some(tenant_id.to_string()))
                .message(format!("restart {name} failed: {e}"))
                .emit();
        }
    }

    /// Per-container resource usage for fleet/dashboard (None when the
    /// container doesn't exist). First poll per boot reports cpu 0.0.
    pub async fn container_stats(&self, tenant_id: &str) -> Option<ContainerStats> {
        self.docker.stats_one(tenant_id).await
    }

    /// Apparent size (bytes) of a tenant's data folder. None when missing
    /// (pure `stat`, no fork — safe on hot paths).
    pub fn disk_size_bytes(&self, tenant_id: &str) -> Option<u64> {
        let path = self.data_disk_path(tenant_id).ok()?;
        Some(dir_size_bytes(&path))
    }

    /// Real host blocks consumed by the tenant data folder. Same as size
    /// (bind folders are not sparse images). Keep off hot paths.
    pub async fn disk_used_bytes(&self, tenant_id: &str) -> Option<u64> {
        let path = self.data_disk_path(tenant_id).ok()?;
        if !path.exists() {
            return None;
        }
        Some(dir_size_bytes(&path))
    }
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



    /// Sync the agent system prompt into the tenant data folder (always
    /// refresh, `.bak-date` backup). Plain host write — works whether the
    /// container runs or not. Returns 1 when written.
    pub async fn sync_workspace_seed(
        &self,
        tenant_id: &str,
        _force: bool,
    ) -> anyhow::Result<usize> {
        let dir = self.data_disk_path(tenant_id)?;
        if !dir.exists() {
            anyhow::bail!("no data folder for tenant");
        }
        let mut written = 0usize;
        // Agent system prompt (managed file: always refresh from the resolved
        // tenant > category > global > host-file source, backing up first).
        let (prompt, source) =
            resolve_system_prompt(&self.db, &self.host_system_prompt_path(), tenant_id).await;
        if !prompt.is_empty() {
            // Outside zerowrapper/workspace: hidden from the agent.
            let zws = dir.join("zerowrapper");
            let _ = std::fs::create_dir_all(&zws);
            let dst = zws.join("SYSTEM.md");
            if dst.exists() {
                let bak = format!(
                    "{}.bak-{}",
                    dst.display(),
                    chrono::Utc::now().format("%Y%m%d")
                );
                let _ = std::fs::copy(&dst, &bak);
            }
            if std::fs::write(&dst, &prompt).is_ok() {
                written += 1;
                self.telemetry
                    .builder()
                    .level(telemetry::Level::Info)
                    .kind("vm.disk.prompt_synced")
                    .category("vm")
                    .data("source", source.clone())
                    .message(format!("synced SYSTEM.md ({source}) for tenant {tenant_id}"))
                    .emit();
            }
        }
        Ok(written)
    }

    /// Drop the guest's binaries version stamp (`versions.json`) so the next
    /// boot re-demands every binary from the wrapper. This is the admin
    /// "push binaries" path: binaries otherwise NEVER move on their own (no
    /// auto-sync — host CPU/RAM). Plain host delete, container may run.
    /// Returns true when a stamp was removed.
    pub async fn drop_binaries_versions(&self, tenant_id: &str) -> anyhow::Result<bool> {
        let dir = self.data_disk_path(tenant_id)?;
        if !dir.exists() {
            anyhow::bail!("no data folder for tenant");
        }
        let stamp = dir.join("supervisor/binaries/versions.json");
        Ok(stamp.exists() && std::fs::remove_file(&stamp).is_ok())
    }

    /// Stop a tenant's container (SIGTERM, docker default timeout) so an
    /// offline operation can proceed. Container + data folder untouched;
    /// RAM state is discarded. Best-effort and idempotent.
    pub async fn kill_vm_process(&self, tenant_id: &str) {
        self.docker.stop(tenant_id).await;
    }

    /// Ensure the tenant's VM is running; return its `IP:port`.
    /// Serialized per-tenant: a browser refresh fires 6 parallel proxy
    /// requests for the same tenant — without the lock they all race into
    /// `fresh_boot` and the second hits `UNIQUE constraint failed: networks.id`.
    /// The hot `live_vm` check runs lock-free first (fast path); the lock
    /// is taken only when a boot/restore is actually needed, then re-checks.
    pub async fn ensure_running(&self, tenant_id: &str) -> anyhow::Result<String> {
        // Fast path: already running, no lock needed.
        if let Some(addr) = self.live_vm(tenant_id).await? {
            return Ok(addr);
        }
        // Acquire per-tenant boot lock.
        let lock = {
            let mut map = self.boot_locks.lock().await;
            map.entry(tenant_id.to_string())
                .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().await;
        // Re-check after acquiring lock — another waiter may have booted it.
        if let Some(addr) = self.live_vm(tenant_id).await? {
            return Ok(addr);
        }
        if let Some(addr) = self.restore_if_snapshotted(tenant_id).await? {
            return Ok(addr);
        }
        self.fresh_boot(tenant_id).await
    }

    /// Snapshot the tenant's live VM to disk. Returns true when a usable
    /// snapshot now exists (idle path calls this before killing the VM and
    /// then KEEPS the row so wake restores instead of rebooting).
    /// Sleep the tenant container (`pause`, cgroup freezer): RAM frozen in
    /// place, zero CPU, instant `unpause` resume. Returns true when the
    /// container is now paused. The idle path calls this and KEEPS the row
    /// so wake unpauses instead of recreating.
    pub async fn snapshot_tenant(&self, tenant_id: &str) -> anyhow::Result<bool> {
        let conn = self.db.conn()?;
        let rows = db::global::networks::all(&conn).await?;
        if rows.iter().all(|r| r.tenant_id != tenant_id) {
            return Ok(false);
        }
        Ok(self.docker.pause(tenant_id).await)
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

    /// Resume a paused container (same IP, RAM intact). Returns None when
    /// there is no row or nothing paused (caller falls back to fresh boot).
    async fn restore_if_snapshotted(
        &self,
        tenant_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let conn = self.db.conn()?;
        let rows = db::global::networks::all(&conn).await?;
        let Some(res) = rows.into_iter().find(|r| r.tenant_id == tenant_id) else {
            return Ok(None);
        };
        let (_running, paused) = self.docker.state(tenant_id).await;
        if !paused {
            return Ok(None);
        }
        if !self.docker.unpause(tenant_id).await {
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
            .message(format!("container unpaused for tenant {tenant_id} at {addr}"))
            .emit();
        Ok(Some(addr))
    }

    /// Full fresh boot for a tenant with no usable container: create (or
    /// reuse) the container and start it. Reuses the kept IP reservation
    /// (kill keeps the row by design); allocates only for tenants that never
    /// had an address. Tenant IPs are stable for life.
    async fn fresh_boot(&self, tenant_id: &str) -> anyhow::Result<String> {
        // Look up the tenant to find their category/template (memory/CPU).
        let tenant = self.tenant(tenant_id).await?;
        let template = self.template_for_tenant(&tenant).await?;
        let memory_mb = if template.memory_mb == 0 {
            DEFAULT_MEM_MIB
        } else {
            template.memory_mb
        };
        let cpus = if template.vcpu_count == 0 {
            DEFAULT_VCPU as f64
        } else {
            template.vcpu_count as f64
        };

        let kept_ip = if let Ok(conn) = self.db.conn() {
            db::global::networks::all(&conn)
                .await
                .ok()
                .and_then(|rows| {
                    rows.into_iter()
                        .find(|r| r.tenant_id == tenant_id)
                        .map(|r| r.ip_address)
                })
        } else {
            None
        };
        let ip = match kept_ip {
            Some(ip) => {
                // Pool is in-memory: re-mark after wrapper restarts.
                self.ip_pool.mark_taken(tenant_id, &ip);
                ip
            }
            None => self
                .ip_pool
                .allocate(tenant_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("IP pool exhausted"))?,
        };

        // Tenant bridge network (idempotent) + data folder (created once).
        self.docker
            .ensure_network(&self.subnet, &self.gateway)
            .await?;
        let data_dir = match self.ensure_data_disk(tenant_id).await {
            Ok(p) => p,
            Err(e) => {
                self.telemetry
                    .builder()
                    .level(telemetry::Level::Warn)
                    .kind("vm.disk.ensure_failed")
                    .category("vm")
                    .tenant_id(Some(tenant_id.to_string()))
                    .message(format!("data folder unavailable, aborting boot: {e}"))
                    .emit();
                return Err(anyhow::anyhow!("data folder unavailable: {e}"));
            }
        };
        self.docker
            .create(
                tenant_id,
                &ip,
                &data_dir.to_string_lossy(),
                memory_mb,
                cpus,
            )
            .await?;
        self.docker
            .start(tenant_id)
            .await?;

        // Record the network assignment + address in the DB. The tap column
        // keeps the container name (same deterministic mapping as before,
        // now pointing at docker instead of a tap device).
        let cname = DockerDriver::container_name(tenant_id);
        let conn = self.db.conn()?;
        db::global::networks::reserve(&conn, tenant_id, &ip, &cname).await?;
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

/// Fast TCP probe (800ms) — true when something accepts on `IP:port`.
async fn probe_tcp(addr: &str) -> bool {
    let fut = tokio::net::TcpStream::connect(addr);
    tokio::time::timeout(std::time::Duration::from_millis(800), fut)
        .await
        .is_ok_and(|r| r.is_ok())
}
