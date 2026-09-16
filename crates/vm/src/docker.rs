//! Docker tenant backend: one container per tenant, no KVM needed.
//!
//! Replaces the Firecracker driver 1:1 at the `VmRuntime` seam:
//! wake/create+start (~0.5s), Sleep = `pause` (RAM frozen, instant resume),
//! Stop = `stop`, Kill = `rm` + recreate. Tenant data is a host folder
//! bind-mounted at `/data` (no loop/ext4). The supervisor inside runs
//! unchanged (PID 1, demand flows identical).

use std::collections::HashMap;
use std::sync::Arc;

use bollard::models::{
    ContainerCreateBody, ContainerCpuStats, ContainerCpuUsage, ContainerMemoryStats,
    ContainerStatsResponse, EndpointIpamConfig, EndpointSettings, HostConfig, Ipam, IpamConfig,
    NetworkCreateRequest, NetworkingConfig,
};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder, StatsOptionsBuilder,
    StopContainerOptionsBuilder,
};
use bollard::Docker;
use futures_util::StreamExt;

/// Per-container resource usage (fleet/dashboard), docker-stats shaped.
#[derive(Debug, Clone, Default)]
pub struct ContainerStats {
    pub cpu_percent: f64,
    pub mem_used_bytes: u64,
    pub mem_limit_bytes: u64,
}

/// Thin async handle around a shared bollard client + last-CPU samples.
#[derive(Clone)]
pub struct DockerDriver {
    docker: Docker,
    image: String,
    network: String,
    last_cpu: Arc<tokio::sync::Mutex<HashMap<String, (u64, u64)>>>,
}

impl DockerDriver {
    pub fn connect(image: impl Into<String>, network: impl Into<String>) -> anyhow::Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| anyhow::anyhow!("connect to local docker: {e}"));
        Ok(Self {
            docker,
            image: image.into(),
            network: network.into(),
            last_cpu: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }

    /// Deterministic container name per tenant (also the DNS name on the
    /// tenant bridge). UUID charset is docker-name safe.
    pub fn container_name(tenant_id: &str) -> String {
        format!("zw-{tenant_id}")
    }

    /// Ensure the tenant bridge network exists (subnet set at creation).
    pub async fn ensure_network(&self, subnet: &str, gateway: &str) -> anyhow::Result<()> {
        match self.docker.inspect_network(&self.network, None).await {
            Ok(_) => Ok(()),
            Err(_) => {
                let res = self
                    .docker
                    .create_network(NetworkCreateRequest {
                        name: self.network.clone(),
                        driver: Some("bridge".to_string()),
                        ipam: Some(Ipam {
                            config: Some(vec![IpamConfig {
                                subnet: Some(subnet.to_string()),
                                gateway: Some(gateway.to_string()),
                                ..Default::default()
                            }]),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })
                    .await;
                match res {
                    Ok(_) => Ok(()),
                    Err(e) if e.to_string().contains("already exists") => Ok(()),
                    Err(e) => Err(anyhow::anyhow!("create network {}: {e}", self.network)),
                }
            }
        }
    }

    /// Container state: (running, paused). Missing container = (false, false).
    pub async fn state(&self, tenant_id: &str) -> (bool, bool) {
        let name = Self::container_name(tenant_id);
        match self.docker.inspect_container(&name, None).await {
            Ok(info) => {
                let st = info.state.as_ref();
                let running = st.and_then(|s| s.running).unwrap_or(false);
                let paused = st.and_then(|s| s.paused).unwrap_or(false);
                (running && !paused, paused)
            }
            Err(_) => (false, false),
        }
    }

    /// Create the tenant container (idempotent: existing name is reused).
    /// Binds the tenant data dir at /data, pins the stable IP, applies
    /// template memory/CPU limits. Never auto-restarts: the WRAPPER owns
    /// lifecycle (wake-on-request), never dockerd.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        tenant_id: &str,
        ip: &str,
        tenant_data_dir: &str,
        memory_mb: u32,
        cpus: f64,
    ) -> anyhow::Result<()> {
        let name = Self::container_name(tenant_id);
        let mut endpoints = HashMap::new();
        endpoints.insert(
            self.network.clone(),
            EndpointSettings {
                ipam_config: Some(EndpointIPAMConfig {
                    ipv4_address: Some(ip.to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        // No restart policy set: docker default is "no" — the WRAPPER owns
        // lifecycle (wake-on-request), never dockerd.
        let config = ContainerCreateBody {
            image: Some(self.image.clone()),
            env: Some(vec![format!("TENANT_ID={tenant_id}")]),
            host_config: Some(HostConfig {
                binds: Some(vec![format!("{tenant_data_dir}:/data:rw")]),
                memory: Some(memory_mb as i64 * 1024 * 1024),
                nano_cpus: Some((cpus * 1_000_000_000.0) as i64),
                ..Default::default()
            }),
            networking_config: Some(NetworkingConfig {
                endpoints_config: endpoints,
            }),
            ..Default::default()
        };
        let options = CreateContainerOptionsBuilder::default()
            .name(&name)
            .build();
        match self.docker.create_container(Some(options), config).await {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("already exists")
                || e.to_string().contains("Conflict") =>
            {
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("create container {name}: {e}")),
        }
    }

    pub async fn start(&self, tenant_id: &str) -> anyhow::Result<()> {
        let name = Self::container_name(tenant_id);
        self.docker
            .start_container(&name, None)
            .await
            .map_err(|e| anyhow::anyhow!("start {name}: {e}"))
    }

    pub async fn stop(&self, tenant_id: &str) {
        let name = Self::container_name(tenant_id);
        let options = StopContainerOptionsBuilder::default().t(10).build();
        let _ = self.docker.stop_container(&name, Some(options)).await;
    }

    pub async fn pause(&self, tenant_id: &str) -> bool {
        let name = Self::container_name(tenant_id);
        self.docker.pause_container(&name).await.is_ok()
    }

    pub async fn unpause(&self, tenant_id: &str) -> bool {
        let name = Self::container_name(tenant_id);
        self.docker.unpause_container(&name).await.is_ok()
    }

    /// Stop + remove the container. The tenant data dir is NEVER touched
    /// (disk persists across Kill by design).
    pub async fn destroy(&self, tenant_id: &str) {
        let name = Self::container_name(tenant_id);
        self.stop(tenant_id).await;
        let options = RemoveContainerOptionsBuilder::default()
            .force(true)
            .build();
        let _ = self.docker.remove_container(&name, Some(options)).await;
        self.last_cpu.lock().await.remove(&name);
    }

    /// One-shot resource usage for fleet/dashboard. CPU% needs two samples:
    /// first poll per boot reports 0.0, subsequent polls the real delta.
    pub async fn stats_one(&self, tenant_id: &str) -> Option<ContainerStats> {
        let name = Self::container_name(tenant_id);
        let options = StatsOptionsBuilder::default()
            .stream(false)
            .one_shot(true)
            .build();
        let mut stream = self.docker.stats(&name, Some(options));
        let stat = stream.next().await?.ok()?;
        let mem_used = stat.memory_stats.as_ref()?.usage.unwrap_or(0);
        let mem_limit = stat.memory_stats.as_ref()?.limit.unwrap_or(0);
        let total = stat
            .cpu_stats
            .as_ref()?
            .cpu_usage
            .as_ref()?
            .total_usage
            .unwrap_or(0);
        let system = stat
            .cpu_stats
            .as_ref()?
            .system_cpu_usage
            .unwrap_or(0);
        let online = stat
            .cpu_stats
            .as_ref()?
            .online_cpus
            .unwrap_or(1)
            .max(1) as f64;
        let mut guard = self.last_cpu.lock().await;
        let (pt, ps) = guard.get(&name).copied().unwrap_or((total, system));
        guard.insert(name, (total, system));
        let cpu_delta = total.saturating_sub(pt) as f64;
        let sys_delta = system.saturating_sub(ps) as f64;
        let cpu_percent = if sys_delta > 0.0 {
            cpu_delta / sys_delta * online * 100.0
        } else {
            0.0
        };
        Some(ContainerStats {
            cpu_percent,
            mem_used_bytes: mem_used,
            mem_limit_bytes: mem_limit,
        })
    }
}
