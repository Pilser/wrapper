//! `wrapper` — Firecracker microVM orchestrator for tenants.
//!
//! Startup flow (ARCHITECTURE.txt STARTUP FLOW, order matters):
//! 1. config.toml → 2. telemetry → 3. logging → 4. db global → 5. db tenant →
//! 6. vm templates → 7. vm manager → 8. network pool →
//! 9. supervisor_server::start (BEFORE proxy) → 10. auth → 11. proxy::start →
//! 12. admin → 13. plugin_host → 14. watcher → 15. telemetry broadcast →
//! 16. event loop until shutdown.
//!
//! NOTE on coverage: the `wrapper` binary depends directly on `api`, `core`
//! and `telemetry` only. Every step below therefore goes through the closest
//! real public API reachable from those crates (mostly the `api::*Api`
//! facades, which delegate to the underlying service crates):
//! - steps 4/5 use `api::db::DbApi::open_global` (runs global migrations) plus
//!   `create_dir_all` for the tenants dir (what `TenantDb::open_in` does per
//!   tenant); there is no `db::global::init` / `db::tenant::init` symbol.
//! - steps 6/7 use `api::vm::VmApi::service` (`vm::VmService::new` +
//!   `health()`); `vm::templates::TemplateRegistry::new` and
//!   `vm::manager::VmManager::with_telemetry` live in the `vm` crate, which is
//!   not a direct dependency of this binary.
//! - step 8 uses `api::network::NetworkApi::manager`
//!   (`network::NetworkManager::with_telemetry`, which builds the `IpPool`).
//! - step 9 builds the real `supervisor_server::ServerConfig` (re-exported via
//!   `api::supervisor`) and logs its addr. Actually serving
//!   (`ServerConfig::start`) needs `db::Db` + `auth::AuthService` values, which
//!   cannot be constructed without direct `db`/`auth` deps, so it is NOT
//!   called here — this is the documented gap.
//! - step 11 builds `api::proxy::ProxyApi` (hook registry) and logs the bind
//!   addr. Actually serving (`proxy::ProxyServer::new(bind,
//!   telemetry).start(shutdown)`) likewise needs a direct `proxy` dep.
//! - steps 13/14 create the plugins dir and log the watcher config; the real
//!   `plugin_host::PluginHost::with_telemetry(...).init()` and
//!   `FolderWatcher` live in the `plugin_host` crate (not a direct dep).
//! - step 15 uses the real `telemetry.subscribe()` broadcast path.

mod config;
mod logging;
mod shutdown;

use telemetry::{Level, Telemetry};

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    // 1. Load config.toml.
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());
    let cfg = config::load(&config_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // 2. Initialize telemetry.
    let telemetry = Telemetry::init(cfg.telemetry.settings());
    api::api_emit!(telemetry, "system.startup", "wrapper starting");
    telemetry
        .builder()
        .level(Level::Info)
        .kind("system.config_loaded")
        .category("system")
        .source("wrapper::main")
        .message(format!("config loaded from {config_path}"))
        .emit();

    // 3. Initialize logging.
    logging::init();

    // 4. db::global::init() → open global DB, run migrations.
    let db_api = api::db::DbApi::new(telemetry.clone());
    let gdb = db_api
        .open_global(&cfg.database.global_path)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("db.query.completed")
        .category("db")
        .source("wrapper::main")
        .message(format!("global db opened at {}", cfg.database.global_path))
        .emit();

    // 5. db::tenant::init() → ensure tenants/ folder exists.
    std::fs::create_dir_all(&cfg.database.tenants_dir)
        .map_err(|e| anyhow::anyhow!(format!("tenants dir: {e}")))?;

    // 5b. Seed global launcher scripts from the host launchers dir
    // (deployment/launchers/{zw,engram}.sh) when the DB rows are empty.
    // Never overwrites portal edits. Supervisors demand these at boot.
    {
        let candidates = [
            std::path::PathBuf::from("../launchers"),
            std::path::PathBuf::from("deployment/launchers"),
        ];
        if let Ok(conn) = gdb.conn() {
            for dir in candidates {
                if !dir.join("zw.sh").exists() && !dir.join("engram.sh").exists() {
                    continue;
                }
                match db::global::launchers::seed_global_if_empty(
                    &conn,
                    &dir,
                    &shared::now_rfc3339(),
                )
                .await
                {
                    Ok(0) => {}
                    Ok(n) => telemetry
                        .builder()
                        .level(Level::Info)
                        .kind("db.launchers_seeded")
                        .category("db")
                        .source("wrapper::main")
                        .message(format!("seeded {n} global launcher scripts from {}", dir.display()))
                        .emit(),
                    Err(e) => telemetry
                        .builder()
                        .level(Level::Warn)
                        .kind("db.launchers_seed_failed")
                        .category("db")
                        .source("wrapper::main")
                        .message(format!("launcher seed failed: {e}"))
                        .emit(),
                }
                break;
            }
        }
    }

    // 6. vm::templates::init() → load registered VM templates.
    // 7. vm::manager::init() → check Firecracker binary.
    let vm_api = api::vm::VmApi::new(telemetry.clone());
    let vm_service = vm_api.service();
    let vm_healthy = vm_service.health();
    telemetry
        .builder()
        .level(Level::Info)
        .kind("vm.template.registered")
        .category("vm")
        .source("wrapper::main")
        .message(format!(
            "vm subsystem ready (docker image={}, healthy={vm_healthy})",
            cfg.docker.image
        ))
        .emit();

    // 8. network::ip_pool::init() → load IP range from config.
    let net_cfg = shared::NetworkConfig {
        ip_range: cfg.network.ip_range.clone(),
        gateway: cfg.network.gateway.clone(),
        netmask: String::new(),
        max_tenants: 1024,
    };
    let net_api = api::network::NetworkApi::new(telemetry.clone());
    let net_manager = net_api
        .manager(net_cfg.clone())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("network.ip.allocated")
        .category("network")
        .source("wrapper::main")
        .message(format!(
            "network pool ready (range={}, gateway={}, available={})",
            cfg.network.ip_range,
            cfg.network.gateway,
            net_manager.ip_pool.available()
        ))
        .emit();

    // 9. supervisor_server::start() → listen for supervisor connections.
    // Serves db_read/db_write envelopes for in-VM supervisor MCP tools.
    let supervisor_config = api::supervisor::ServerConfig {
        bind_address: cfg.supervisor_server.bind_address.clone(),
        port: cfg.supervisor_server.port,
        heartbeat_timeout_seconds: cfg.supervisor_server.heartbeat_timeout_seconds,
    };
    let supervisor_api =
        api::supervisor::SupervisorApi::with_config(telemetry.clone(), supervisor_config);
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.connected")
        .category("supervisor_server")
        .source("wrapper::main")
        .message(format!(
            "supervisor listener configured at {}",
            supervisor_api.addr()
        ))
        .emit();
    // One activity map shared by the HTTP API and the supervisor listener:
    // heartbeats feed the same liveness the dashboard and idle math read.
    let activity = proxy::ActivityMap::new();
    {
        let srv_cfg = supervisor_api.config.clone();
        let srv_tel = telemetry.clone();
        let err_tel = telemetry.clone();
        let srv_db = db::Db {
            global: gdb.clone(),
        };
        let srv_auth = auth::AuthService::new(telemetry.clone());
        let srv_activity = activity.clone();
        tokio::spawn(async move {
            if let Err(e) = srv_cfg.start(srv_tel, srv_db, srv_auth, srv_activity).await {
                err_tel.builder()
                    .level(Level::Error)
                    .kind("supervisor.serve.failed")
                    .category("supervisor_server")
                    .message(format!("supervisor listener exited: {e}"))
                    .emit();
            }
        });
    }

    // 10. auth::init() → load auth config.
    let auth_api = api::auth::AuthApi::new(telemetry.clone());
    let _auth = auth_api.service();

    // 11. proxy::start() → start client proxy (after supervisor_server).
    let proxy_api = api::proxy::ProxyApi::new();
    let proxy_bind = format!("{}:{}", cfg.wrapper.bind_address, cfg.wrapper.port);
    let _ = proxy_api.hooks();
    telemetry
        .builder()
        .level(Level::Info)
        .kind("proxy.request.received")
        .category("proxy")
        .source("wrapper::main")
        .message(format!(
            "proxy configured on {proxy_bind} (serve via ProxyServer::start once proxy wired)"
        ))
        .emit();

    // 12. admin::init() → load categories and deployment configs.
    let admin = api::admin::AdminApi::new(telemetry.clone());
    let _admin_service = admin.service();
    match admin.list_categories(&gdb).await {
        Ok(categories) => telemetry
            .builder()
            .level(Level::Info)
            .kind("admin.category.created")
            .category("admin")
            .source("wrapper::main")
            .message(format!("admin ready ({} categories)", categories.len()))
            .emit(),
        Err(e) => telemetry
            .builder()
            .level(Level::Warn)
            .kind("admin.category.created")
            .category("admin")
            .source("wrapper::main")
            .message(format!("admin ready (category list unavailable: {e})"))
            .emit(),
    }

    // 13. plugin_host::init() → load all plugins.
    std::fs::create_dir_all(&cfg.plugins.directory)
        .map_err(|e| anyhow::anyhow!(format!("plugins dir: {e}")))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("plugin.loaded")
        .category("plugin")
        .source("wrapper::main")
        .message(format!(
            "plugin host configured (dir={}, serve via PluginHost::init once plugin_host wired)",
            cfg.plugins.directory
        ))
        .emit();

    // 14. plugin_host::watcher::start() → watch plugins/ for changes.
    telemetry
        .builder()
        .level(Level::Info)
        .kind("plugin.hook.registered")
        .category("plugin")
        .source("wrapper::main")
        .message(format!("plugin watcher configured (hot_reload={})", cfg.plugins.hot_reload))
        .emit();

    // 15. telemetry::broadcast::start() → start live telemetry server.
    let _live = telemetry.subscribe();
    telemetry
        .builder()
        .level(Level::Info)
        .kind("system.startup")
        .category("telemetry")
        .source("wrapper::main")
        .message(format!(
            "telemetry broadcast at {}:{} ({})",
            cfg.telemetry.broadcast.bind_address,
            cfg.telemetry.broadcast.port,
            cfg.telemetry.broadcast.protocol
        ))
        .emit();

    let _api = api::ApiContextBuilder::new()
        .telemetry(telemetry.clone())
        .plugin_name("wrapper-main")
        .build()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    println!(
        "wrapper listening on {}:{} (supervisor {}, deployments push port {})",
        cfg.wrapper.bind_address,
        cfg.wrapper.port,
        supervisor_api.addr(),
        cfg.deployments.default_push_port
    );

    // 15b. Serve the public listener: /api/* (user + admin APIs for the
    // frontend) with everything else proxied to the tenant's zw.
    // Runs as a background task; the event loop below still owns shutdown.
    let serve_bind = format!("{}:{}", cfg.wrapper.bind_address, cfg.wrapper.port);
    // Absolute: docker bind sources must be absolute paths (the daemon
    // rejects relative ones). Resolved against the wrapper cwd.
    let tenant_data_root = std::path::Path::new(&cfg.docker.tenant_data_root)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|c| c.join(&cfg.docker.tenant_data_root).to_string_lossy().to_string())
                .unwrap_or_else(|_| cfg.docker.tenant_data_root.clone())
        });
    let _ = std::fs::create_dir_all(&tenant_data_root);
    let telemetry_dir = std::path::Path::new(&cfg.telemetry.disk.path)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| cfg.telemetry.disk.path.clone());
    let serve_params = api::server::ServeParams {
        gdb: gdb.clone(),
        telemetry: telemetry.clone(),
        activity: activity.clone(),
        telemetry_dir,
        net_cfg: net_cfg.clone(),
        docker_image: cfg.docker.image.clone(),
        docker_network: cfg.docker.network.clone(),
        docker_subnet: cfg.docker.subnet.clone(),
        docker_gateway: cfg.docker.gateway.clone(),
        public_base: cfg.api.public_base.clone(),
        tenant_data_root,
        binaries_dir: cfg.binaries.dir.clone(),
        system_prompt_host_path: cfg.vm.system_prompt_host_path.clone(),
        idle_timeout_minutes: cfg.vm.idle_timeout_minutes,
    };
    let serve_telemetry = telemetry.clone();
    let serve_task = tokio::spawn(async move {
        if let Err(e) = api::server::serve(&serve_bind, serve_params).await {
            serve_telemetry
                .builder()
                .level(Level::Error)
                .kind("api.serve.failed")
                .category("api")
                .source("wrapper::main")
                .message(format!("public listener failed: {e}"))
                .emit();
        }
    });

    // 16. Enter event loop → run until shutdown.
    let shutdown = shutdown::GracefulShutdown::new();
    shutdown::wait_for_signal(&shutdown).await;

    serve_task.abort();
    println!("shutting down...");
    telemetry
        .builder()
        .level(Level::Info)
        .kind("system.shutdown")
        .category("system")
        .source("wrapper::main")
        .message("wrapper shutting down")
        .emit();
    telemetry.flush().await;
    Ok(())
}
