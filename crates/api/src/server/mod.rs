//! Small hyper HTTP API served on the wrapper bind address under `/api/*`.
//!
//! Two namespaces, one server (the same hyper listener also proxies every
//! non-`/api` path to the tenant's zw — see `proxy::HttpHandler`):
//!
//! * `/api/...` — user APIs (session Bearer token from login). Anything else
//!   under a tenant route is proxied to the VM (wake-on-request).
//! * `/api/admin/...` — admin APIs (staff Bearer token, role `admin`).
//!
//! Full endpoint reference for frontend teams:
//! `deployment/API-REFERENCE.txt`.

pub mod admin;
pub mod auth_ctx;
pub mod idle;
pub mod json;
pub mod stream;
pub mod user;

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use telemetry::Telemetry;

use crate::server::idle::ActivityMap;

/// Shared state for every `/api` handler.
#[derive(Clone)]
pub struct AppState {
    pub gdb: db::global::GlobalDb,
    pub telemetry: Telemetry,
    /// VM runtime for wake/pre-warm/shutdown. None in degraded mode.
    pub runtime: Option<vm::runtime::VmRuntime>,
    /// Last-seen per tenant (also fed by the proxy handler).
    pub activity: ActivityMap,
    /// Public base URL for minting serverless links (config `[api]`).
    pub public_base: String,
    /// Absolute host dir with telemetry daily JSON files (config `[telemetry.disk]`).
    pub telemetry_dir: String,
    /// Host root holding one folder per tenant (bind-mounted at `/data`).
    pub tenant_data_root: String,
    /// Host directory with versioned guest binaries + zw assets.
    pub binaries_dir: String,
    /// Host path of the default agent system prompt (config `[vm]`).
    pub system_prompt_host_path: String,
    /// Idle minutes before auto-sleep (config `[vm]`, 0 = disabled).
    pub idle_timeout_minutes: u64,
}

impl AppState {
    /// `apps_url` for a tenant: the public root of their zw area.
    pub fn apps_url(&self, tenant_id: &str) -> String {
        format!(
            "{}/tenant_{tenant_id}/",
            self.public_base.trim_end_matches('/')
        )
    }
}

/// Inputs to serve the wrapper's public listener: `/api/*` locally, every
/// other path proxied to the tenant's zw (wake-on-request).
pub struct ServeParams {
    pub gdb: db::global::GlobalDb,
    pub telemetry: Telemetry,
    /// Shared with the supervisor TCP listener: heartbeats feed the same map.
    pub activity: ActivityMap,
    pub net_cfg: shared::NetworkConfig,
    pub docker_image: String,
    pub docker_network: String,
    pub docker_subnet: String,
    pub docker_gateway: String,
    pub public_base: String,
    /// Host root holding one folder per tenant (bind-mounted at `/data`).
    pub tenant_data_root: String,
    /// Prebuilt container runtime shared with the supervisor TCP listener.
    /// None = build one here (old path, with pool pre-mark).
    pub runtime: Option<vm::runtime::VmRuntime>,
    /// Host dir with versioned guest binaries + zw assets (config
    /// `[binaries]`). Portal uploads land here; supervisors demand them.
    pub binaries_dir: String,
    /// Host path of the default agent system prompt (config `[vm]`).
    pub system_prompt_host_path: String,
    pub idle_timeout_minutes: u64,
    /// Absolute host dir with telemetry daily JSON files (config `[telemetry.disk]`).
    pub telemetry_dir: String,
}

/// Serve forever: one hyper listener, `/api/*` routed locally, everything
/// else through the zw reverse proxy. Spawns the idle reaper when the
/// timeout is non-zero.
pub async fn serve(addr: &str, p: ServeParams) -> anyhow::Result<()> {
    // Shared runtime from main (same instance as the supervisor TCP
    // listener) wins; otherwise build one here (old path). Pre-mark runs
    // exactly once: here only on the old path, in main for a shared one.
    let runtime = match p.runtime {
        Some(rt) => rt,
        None => {
            let docker =
                vm::docker::DockerDriver::connect(&p.docker_image, &p.docker_network)
                    .map_err(|e| anyhow::anyhow!("docker unavailable: {e}"))?;
            let rt = vm::runtime::VmRuntime::new(
                docker,
                p.gdb.clone(),
                p.net_cfg,
                p.telemetry.clone(),
                p.docker_network.clone(),
                p.docker_subnet.clone(),
                p.docker_gateway.clone(),
                p.tenant_data_root.clone(),
            )?
            .with_system_prompt_host_path(p.system_prompt_host_path.clone());
            // Restart recovery BEFORE the runtime moves into handler/state:
            // never re-allocate an address a live VM still holds.
            match rt.premark_reserved().await {
                Ok(n) => p.telemetry
                    .builder()
                    .level(telemetry::Level::Info)
                    .kind("vm.pool.premarked")
                    .category("vm")
                    .message(format!("pool pre-marked with {n} reserved addresses"))
                    .emit(),
                Err(e) => p.telemetry
                    .builder()
                    .level(telemetry::Level::Warn)
                    .kind("vm.pool.premark_failed")
                    .category("vm")
                    .message(format!("pool pre-mark failed: {e}"))
                    .emit(),
            }
            rt
        }
    };
    let activity = p.activity.clone();
    let handler = proxy::HttpHandler::new(p.telemetry.clone(), runtime.clone())
        .with_activity(activity.clone());
    let state = AppState {
        gdb: p.gdb,
        telemetry: p.telemetry.clone(),
        runtime: Some(runtime),
        activity,
        public_base: p.public_base,
        idle_timeout_minutes: p.idle_timeout_minutes,
        tenant_data_root: p.tenant_data_root,
        binaries_dir: p.binaries_dir,
        system_prompt_host_path: p.system_prompt_host_path,
        telemetry_dir: p.telemetry_dir,
    };
    if p.idle_timeout_minutes > 0 {
        idle::spawn_reaper(
            state.clone(),
            std::time::Duration::from_secs(p.idle_timeout_minutes * 60),
        );
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    p.telemetry
        .builder()
        .level(telemetry::Level::Info)
        .kind("api.serve.listening")
        .category("api")
        .message(format!("wrapper serving /api + zw proxy on {addr}"))
        .emit();
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        let handler = handler.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                let state = state.clone();
                let handler = handler.clone();
                async move {
                    // Peek before moving: the wrapper's OWN /api namespaces
                    // are served locally; everything else — including zw's
                    // own /api/* app routes (/api/sessions, ...) — goes to
                    // the tenant's zw (wake-on-request). zw owns its paths;
                    // the wrapper only owns auth/me/admin.
                    // Browsers (Cloudflare Pages frontend) need CORS: answer
                    // preflights and tag every /api response.
                    let path = req.uri().path().to_string();
                    let is_api = path == "/api"
                        || path.starts_with("/api/auth")
                        || path.starts_with("/api/me")
                        || path.starts_with("/api/admin")
                        // In-VM supervisor fetch endpoints (binaries): no
                        // admin session exists inside the guest.
                        || path.starts_with("/api/supervisor");
                    if is_api {
                        if req.method() == hyper::Method::OPTIONS {
                            return Ok::<_, anyhow::Error>(json::cors_preflight());
                        }
                        if let Some(res) = route(&state, req).await {
                            return Ok(json::with_cors(res));
                        }
                        return Ok(json::with_cors(json::not_found("unknown api route")));
                    }
                    Ok(handler.handle(req).await.unwrap_or_else(|e| {
                        json::json(
                            hyper::StatusCode::INTERNAL_SERVER_ERROR,
                            serde_json::json!({"error": e.to_string()}),
                        )
                    }))
                }
            });
            // with_upgrades() is REQUIRED for websocket tunneling: plain
            // serve_connection closes the socket after a 101 response
            // instead of hijacking it (the browser then sees EOF and zw
            // sees a reset). The proxy handler takes the upgrade via
            // hyper::upgrade::on() and pipes both directions.
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .with_upgrades()
                .await;
        });
    }
}

pub type ApiResponse = hyper::Response<Full<Bytes>>;

/// Dispatch one `/api/*` request. Returns None when the path is NOT an API
/// route (caller falls through to the zw proxy).
pub async fn route(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> Option<ApiResponse> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.first() != Some(&"api") {
        return None;
    }
    let res = dispatch(state, method.as_str(), &segs, req).await;
    Some(res)
}

async fn dispatch(
    state: &AppState,
    method: &str,
    segs: &[&str],
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    match (method, segs) {
        // ---- user auth ----
        ("POST", ["api", "auth", "signup"]) => user::signup(state, req).await,
        ("POST", ["api", "auth", "login"]) => user::login(state, req).await,
        ("POST", ["api", "auth", "logout"]) => user::logout(state, req).await,
        // ---- self-service ----
        ("GET", ["api", "me"]) => user::me(state, req).await,
        ("GET", ["api", "me", "vm", "status"]) => user::vm_status(state, req).await,
        ("POST", ["api", "me", "vm", "wake"]) => user::vm_wake(state, req).await,
        // ---- admin bootstrap/staff ----
        ("POST", ["api", "admin", "bootstrap"]) => admin::bootstrap(state, req).await,
        ("POST", ["api", "admin", "login"]) => admin::staff_login(state, req).await,
        ("GET", ["api", "admin", "me"]) => admin::staff_me(state, req).await,
        // ---- admin tenants ----
        ("GET", ["api", "admin", "tenants"]) => admin::tenants_list(state, req).await,
        ("POST", ["api", "admin", "tenants"]) => admin::tenants_create(state, req).await,
        ("GET", ["api", "admin", "tenants", _]) => admin::tenants_get(state, req).await,
        ("PUT", ["api", "admin", "tenants", _]) => admin::tenants_update(state, req).await,
        ("DELETE", ["api", "admin", "tenants", _]) => admin::tenants_delete(state, req).await,
        ("POST", ["api", "admin", "tenants", _, "suspend"]) => {
            admin::tenants_suspend(state, req).await
        }
        ("POST", ["api", "admin", "tenants", _, "unsuspend"]) => {
            admin::tenants_unsuspend(state, req).await
        }
        // ---- admin templates ----
        ("GET", ["api", "admin", "templates"]) => admin::templates_list(state, req).await,
        ("POST", ["api", "admin", "templates"]) => admin::templates_create(state, req).await,
        ("GET", ["api", "admin", "templates", _]) => admin::templates_get(state, req).await,
        ("PUT", ["api", "admin", "templates", _]) => admin::templates_update(state, req).await,
        ("DELETE", ["api", "admin", "templates", _]) => {
            admin::templates_delete(state, req).await
        }
        // ---- admin categories ----
        ("GET", ["api", "admin", "categories"]) => admin::categories_list(state, req).await,
        ("POST", ["api", "admin", "categories"]) => admin::categories_create(state, req).await,
        ("GET", ["api", "admin", "categories", _]) => admin::categories_get(state, req).await,
        ("PUT", ["api", "admin", "categories", _]) => admin::categories_update(state, req).await,
        ("DELETE", ["api", "admin", "categories", _]) => {
            admin::categories_delete(state, req).await
        }
        ("GET", ["api", "admin", "categories", _, "tools"]) => {
            admin::categories_tools_get(state, req).await
        }
        ("PUT", ["api", "admin", "categories", _, "tools"]) => {
            admin::categories_tools_set(state, req).await
        }
        ("GET", ["api", "admin", "tools"]) => admin::tools_catalog(state, req).await,
        // ---- admin binary versions ----
        ("GET", ["api", "admin", "binaries", "versions"]) => admin::binary_versions_list(state, req).await,
        ("POST", ["api", "admin", "binaries", "diff"]) => admin::binaries_diff(state, req).await,
        ("POST", ["api", "admin", "binaries", "merge"]) => admin::binaries_merge(state, req).await,
        ("POST", ["api", "admin", "binaries", "rollback"]) => admin::binaries_rollback(state, req).await,
        ("POST", ["api", "admin", "binaries", "sync"]) => admin::binaries_sync(state, req).await,
        ("POST", ["api", "admin", "binaries", "sync-bulk"]) => admin::binaries_sync_bulk(state, req).await,
        ("GET", ["api", "admin", "binaries", "effective"]) => admin::binaries_effective(state, req).await,
        // ---- admin hub (cross-tenant portfolio grants) ----
        ("GET", ["api", "admin", "hub", "grants"]) => admin::hub_grants_list(state, req).await,
        ("POST", ["api", "admin", "hub", "grants"]) => admin::hub_grants_create(state, req).await,
        ("DELETE", ["api", "admin", "hub", "grants"]) => admin::hub_grants_delete(state, req).await,
        // ---- admin fleet ----
        ("GET", ["api", "admin", "vms"]) => admin::vms_list(state, req).await,
        ("POST", ["api", "admin", "vms", _, "shutdown"]) => {
            admin::vms_shutdown(state, req).await
        }
        ("POST", ["api", "admin", "vms", _, "sleep"]) => {
            admin::vms_sleep(state, req).await
        }
        ("POST", ["api", "admin", "vms", _, "stop"]) => {
            admin::vms_stop(state, req).await
        }
        ("POST", ["api", "admin", "vms", _, "kill"]) => {
            admin::vms_kill(state, req).await
        }
        ("GET", ["api", "admin", "disks"]) => admin::disks_list(state, req).await,
        ("POST", ["api", "admin", "workspace", "sync"]) => {
            admin::workspace_sync(state, req).await
        }
        ("POST", ["api", "admin", "workspace", "sync-bulk"]) => {
            admin::workspace_sync_bulk(state, req).await
        }
        ("GET", ["api", "admin", "system-prompt"]) => {
            admin::system_prompt_get(state, req).await
        }
        ("PUT", ["api", "admin", "system-prompt"]) => {
            admin::system_prompt_set(state, req).await
        }
        ("GET", ["api", "admin", "system-prompt", "effective"]) => {
            admin::system_prompt_effective(state, req).await
        }
        ("DELETE", ["api", "admin", "system-prompt"]) => {
            admin::system_prompt_delete(state, req).await
        }
        ("GET", ["api", "admin", "launchers"]) => {
            admin::launchers_get(state, req).await
        }
        ("PUT", ["api", "admin", "launchers"]) => {
            admin::launchers_set(state, req).await
        }
        ("DELETE", ["api", "admin", "launchers"]) => {
            admin::launchers_delete(state, req).await
        }
        ("GET", ["api", "admin", "binaries"]) => {
            admin::binaries_list(state, req).await
        }
        ("GET", ["api", "supervisor", "binaries", "manifest"]) => {
            admin::supervisor_binaries_manifest(state, req).await
        }
        ("GET", ["api", "supervisor", "binaries", "file"]) => {
            admin::supervisor_binaries_file(state, req).await
        }
        ("PUT", ["api", "admin", "binaries", "file"]) => {
            admin::binaries_upload(state, req).await
        }
        ("POST", ["api", "admin", "binaries", "push"]) => {
            admin::binaries_push(state, req).await
        }
        ("POST", ["api", "admin", "binaries", "push-bulk"]) => {
            admin::binaries_push_bulk(state, req).await
        }
        ("GET", ["api", "admin", "tenants", _, "disk"]) => {
            admin::tenant_disk_show(state, req).await
        }
        ("POST", ["api", "admin", "tenants", _, "disk", "resize"]) => {
            admin::tenant_disk_resize(state, req).await
        }
        ("GET", ["api", "admin", "dashboard"]) => {
            admin::dashboard_snapshot(state, req).await
        }
        ("GET", ["api", "admin", "stream"]) => match stream::admin_stream(state, req).await {
            Ok(r) => r,
            Err(_) => json::internal(),
        },
        ("GET", ["api", "admin", "logs"]) => admin::logs_list(state, req).await,
        ("GET", ["api", "admin", "events"]) => match stream::admin_events(state, req).await {
            Ok(r) => r,
            Err(_) => json::internal(),
        },
        ("GET", ["api", "admin", "categories", _, "configs"]) => {
            admin::category_configs_list(state, req).await
        }
        ("POST", ["api", "admin", "categories", _, "configs"]) => {
            admin::category_configs_create(state, req).await
        }
        ("DELETE", ["api", "admin", "categories", _, "configs", _]) => {
            admin::category_configs_delete(state, req).await
        }
        _ => json::not_found("unknown api route"),
    }
}
