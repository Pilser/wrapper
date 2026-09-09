//! Idle auto-sleep: snapshot quiet VMs so wake restores in ~0.2 s.
//!
//! Activity is touched per forwarded request (proxy handler) and per wake.
//! The reaper runs every 60s: a tenant quiet longer than `idle_timeout`
//! gets `shutdown_tenant`:
//!   1. snapshot the live VM to disk (`snapshots/<tenant>/{vmstate,mem}`);
//!   2. kill the FC process, delete the tap;
//!   3. KEEP the networks row + IP, mark tenant Asleep.
//! Wake then restores from disk (same IP/MAC/tap) instead of rebooting.
//! If snapshotting fails, fall back to the old kill + row-release path.

use std::time::Duration;

use super::AppState;

/// Shared activity handle (re-exported so proxy + server use one type).
pub type ActivityMap = proxy::routing::ActivityMap;

/// Sleep one tenant's VM (snapshot-first) and clear activity tracking.
/// Idempotent: missing process/tap/row/snapshot is success, not error.
pub async fn shutdown_tenant(state: &AppState, tenant_id: &str) -> anyhow::Result<()> {
    // 1. Find the reservation (need tap name + socket path).
    let conn = state.gdb.conn()?;
    let rows = db::global::networks::all(&conn).await?;
    let tap = rows
        .iter()
        .find(|r| r.tenant_id == tenant_id)
        .map(|r| r.tap_device.clone());

    // 2. Snapshot-first when a runtime is configured: keep the row on
    //    success so wake restores (fast path).
    let mut snapshotted = false;
    if let Some(runtime) = &state.runtime {
        match runtime.snapshot_tenant(tenant_id).await {
            Ok(true) => snapshotted = true,
            Ok(false) => {}
            Err(e) => {
                state
                    .telemetry
                    .builder()
                    .level(telemetry::Level::Warn)
                    .kind("vm.sleep.snapshot_failed")
                    .category("vm")
                    .tenant_id(Some(tenant_id.to_string()))
                    .message(format!("snapshot failed, killing instead: {e}"))
                    .emit();
            }
        }
    }

    // 3. SIGTERM the Firecracker process by its deterministic API socket.
    let sock_pattern = format!("/tmp/fc_{tenant_id}.sock");
    let _ = tokio::process::Command::new("pkill")
        .args(["-f", &sock_pattern])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;

    // 4. Remove the tap device (best-effort; needs root). It is recreated
    //    with the identical name on restore (snapshot references it).
    if let Some(tap) = tap {
        let _ = tokio::process::Command::new("ip")
            .args(["link", "del", "dev", &tap])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
    }

    // 5. DB: snapshotted -> KEEP row + IP, mark Asleep (wake restores).
    //    Otherwise release everything so the next request fresh-boots.
    //    Status uses a targeted write (never a full-row write from a
    //    possibly stale read-back: lost-update hazard).
    if snapshotted {
        let _ = db::global::tenants::set_status(
            &conn,
            tenant_id,
            shared::TenantStatus::Asleep.as_str(),
        )
        .await;
    } else {
        let _ = db::global::networks::release_all(&conn, tenant_id).await;
        if let Ok(Some(mut t)) = db::global::tenants::get_by_id(&conn, tenant_id).await {
            t.vm_ip = None;
            t.tap_device = None;
            t.vm_process_id = None;
            let _ = db::global::tenants::update(&conn, &t).await;
        }
        let _ = db::global::tenants::set_status(
            &conn,
            tenant_id,
            shared::TenantStatus::Asleep.as_str(),
        )
        .await;
    }
    state.activity.remove(tenant_id);

    state
        .telemetry
        .builder()
        .level(telemetry::Level::Info)
        .kind("vm.sleep.completed")
        .category("vm")
        .tenant_id(Some(tenant_id.to_string()))
        .message(format!(
            "VM asleep for tenant {tenant_id} (snapshotted={snapshotted})"
        ))
        .emit();
    Ok(())
}

/// Spawn the reaper loop. `idle_timeout` of ZERO disables auto-sleep.
pub fn spawn_reaper(state: AppState, idle_timeout: Duration) {
    if idle_timeout.is_zero() {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
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
