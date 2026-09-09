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
    /// Host directory scanned by GET /api/admin/disks (guest images).
    pub disks_dir: String,
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
    pub firecracker_bin: String,
    pub net_cfg: shared::NetworkConfig,
    pub kernel_path: String,
    pub initrd_path: String,
    pub public_base: String,
    pub disks_dir: String,
    pub idle_timeout_minutes: u64,
}

/// Serve forever: one hyper listener, `/api/*` routed locally, everything
/// else through the zw reverse proxy. Spawns the idle reaper when the
/// timeout is non-zero.
pub async fn serve(addr: &str, p: ServeParams) -> anyhow::Result<()> {
    let runtime = vm::runtime::VmRuntime::new(
        expand_home(&p.firecracker_bin),
        p.gdb.clone(),
        p.net_cfg,
        p.telemetry.clone(),
        expand_home(&p.kernel_path),
        expand_home(&p.initrd_path),
    )?;
    let activity = ActivityMap::new();
    let handler = proxy::HttpHandler::new(p.telemetry.clone(), runtime.clone())
        .with_activity(activity.clone());
    let state = AppState {
        gdb: p.gdb,
        telemetry: p.telemetry.clone(),
        runtime: Some(runtime),
        activity,
        public_base: p.public_base,
        idle_timeout_minutes: p.idle_timeout_minutes,
        disks_dir: p.disks_dir,
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
                    // Peek before moving: /api/* is served locally, every
                    // other path goes to the tenant's zw (wake-on-request).
                    let path = req.uri().path().to_string();
                    let is_api = path == "/api" || path.starts_with("/api/");
                    if is_api {
                        if let Some(res) = route(&state, req).await {
                            return Ok::<_, anyhow::Error>(res);
                        }
                        return Ok(json::not_found("unknown api route"));
                    }
                    Ok(handler.handle(req).await.unwrap_or_else(|e| {
                        json::json(
                            hyper::StatusCode::INTERNAL_SERVER_ERROR,
                            serde_json::json!({"error": e.to_string()}),
                        )
                    }))
                }
            });
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .await;
        });
    }
}

/// Expand a leading `~/` with $HOME (config paths like the firecracker bin).
fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
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
        // ---- admin fleet ----
        ("GET", ["api", "admin", "vms"]) => admin::vms_list(state, req).await,
        ("POST", ["api", "admin", "vms", _, "shutdown"]) => {
            admin::vms_shutdown(state, req).await
        }
        ("GET", ["api", "admin", "disks"]) => admin::disks_list(state, req).await,
        _ => json::not_found("unknown api route"),
    }
}
