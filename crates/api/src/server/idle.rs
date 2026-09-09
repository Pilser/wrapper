//! Idle auto-sleep: reap quiet VMs so the fleet scales to zero.
//!
//! Activity is touched per forwarded request (proxy handler) and per wake.
//! The reaper runs every 60s: a tenant quiet longer than `idle_timeout`
//! gets `shutdown_tenant`. Shutdown MUST clear the `networks` row (and the
//! tenant's vm fields) so the next request re-wakes via `ensure_running`
//! instead of trusting a stale address.
//!
//! Guest shutdown = SIGTERM the Firecracker process (the initramfs guest has
//! no ACPI handler; SendCtrlAltDel would reboot it). The tap device is
//! removed and the IP returns to the pool implicitly (reservations table).

use std::time::Duration;

use super::AppState;

/// Shared activity handle (re-exported so proxy + server use one type).
pub type ActivityMap = proxy::routing::ActivityMap;

/// Shutdown one tenant's VM and clear every trace of the assignment.
/// Idempotent: missing process/tap/row is success, not error.
pub async fn shutdown_tenant(state: &AppState, tenant_id: &str) -> anyhow::Result<()> {
    // 1. Find the reservation (need tap name).
    let conn = state.gdb.conn()?;
    let rows = db::global::networks::all(&conn).await?;
    let tap = rows
        .iter()
        .find(|r| r.tenant_id == tenant_id)
        .map(|r| r.tap_device.clone());

    // 2. SIGTERM the Firecracker process by its deterministic API socket.
    //    spawn uses --api-sock /tmp/fc_<tenant>.sock, unique per tenant.
    let sock_pattern = format!("/tmp/fc_{tenant_id}.sock");
    let _ = tokio::process::Command::new("pkill")
        .args(["-f", &sock_pattern])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;

    // 3. Remove the tap device (best-effort; needs root).
    if let Some(tap) = tap {
        let _ = tokio::process::Command::new("ip")
            .args(["link", "del", "dev", &tap])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
    }

    // 4. Clear DB traces so the next request re-wakes (never stale).
    let _ = db::global::networks::release_all(&conn, tenant_id).await;
    if let Ok(Some(mut t)) = db::global::tenants::get_by_id(&conn, tenant_id).await {
        t.vm_ip = None;
        t.tap_device = None;
        t.vm_process_id = None;
        t.status = shared::TenantStatus::Asleep;
        let _ = db::global::tenants::update(&conn, &t).await;
    }
    state.activity.remove(tenant_id);

    state
        .telemetry
        .builder()
        .level(telemetry::Level::Info)
        .kind("vm.sleep.completed")
        .category("vm")
        .tenant_id(Some(tenant_id.to_string()))
        .message(format!("VM asleep for tenant {tenant_id}"))
        .emit();
    Ok(())
}

/// Spawn the reaper loop. `idle_timeout` of ZERO disables auto-sleep.
pub fn spawn_reaper(state: AppState, idle_timeout: Duration) {
    if idle_timeout.is_zero() {
        return;
    }
    tokio::spawn(async move {
        let mut tick =
            tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            for (tenant_id, last) in state.activity.snapshot() {
                if last.elapsed() >= idle_timeout {
                    let _ = shutdown_tenant(&state, &tenant_id).await;
                }
            }
            // Also reap reservations with NO recorded activity at all
            // (VMs booted before this process started): treat the
            // reservation as active once so they get a full timeout.
            if let Ok(conn) = state.gdb.conn() {
                if let Ok(rows) = db::global::networks::all(&conn).await {
                    for row in rows {
                        if state.activity.last(&row.tenant_id).is_none() {
                            state.activity.touch(&row.tenant_id);
                        }
                    }
                }
            }
        }
    });
}
