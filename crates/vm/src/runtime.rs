//! Wake-on-request runtime: ensure a tenant's VM is running.
//!
//! `VmRuntime::ensure_running(tenant_id)` is the single entry point the proxy
//! calls before forwarding a request. It returns the VM's `IP:port` (the
//! zerowrapper port). If the tenant already has a reachable VM it returns its
//! address; otherwise it assigns an IP, creates a tap device, boots Firecracker,
//! and records everything in the DB.

use shared::{NetworkConfig, Tenant, VmTemplate};
use telemetry::Telemetry;

use crate::firecracker::FirecrackerClient;
use crate::manager::VmManager;

// Network helpers live in the `network` crate; re-exported here for convenience.
use network::ip::IpPool;
use network::tap::{self, TapCreateOptions};

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
        })
    }

    /// Ensure the tenant's VM is running; return its `IP:port`.
    pub async fn ensure_running(&self, tenant_id: &str) -> anyhow::Result<String> {
        // 1. If the tenant already has a recorded VM, return it.
        if let Some(addr) = self.existing_vm(tenant_id).await? {
            return Ok(addr);
        }

        // 2. Look up the tenant to find their category/template.
        let tenant = self.tenant(tenant_id).await?;
        let template = self.template_for_tenant(&tenant).await?;

        // 3. Assign an IP from the pool.
        let ip = self
            .ip_pool
            .allocate(tenant_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("IP pool exhausted"))?;

        // 4. Create + attach a tap device.
        let tap = format!("tap_{}", &tenant_id[..8]);
        tap::create_tap(TapCreateOptions {
            name: tap.clone(),
            ip: ip.clone(),
        })?;
        tap::configure_tap(&tap, &ip)?;

        // 5. Boot Firecracker for this tenant.
        let sock = std::path::PathBuf::from(format!("/tmp/fc_{tenant_id}.sock"));
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
        crate::lifecycle::boot::boot_vm(&self.telemetry, &client, tenant_id, &boot_template)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // 6. Record the network assignment + VM process in the DB.
        let conn = self.db.conn()?;
        db::global::networks::reserve(&conn, tenant_id, &ip, &tap).await?;
        db::global::tenants::set_vm_address(&conn, tenant_id, &ip).await?;

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

    /// Return the existing `IP:port` if the tenant already has a recorded VM.
    async fn existing_vm(&self, tenant_id: &str) -> anyhow::Result<Option<String>> {
        let conn = self.db.conn()?;
        let rows = db::global::networks::all(&conn).await?;
        if let Some(res) = rows.into_iter().find(|r| r.tenant_id == tenant_id) {
            return Ok(Some(format!("{ip}:{port}", ip = res.ip_address, port = ZERO_WRAPPER_PORT)));
        }
        Ok(None)
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
}
