//! Idle auto-sleep: pause quiet containers so wake unpauses instantly.
//!
//! Activity is touched per forwarded request (proxy handler) and per wake.
//! The reaper runs every 60s: a tenant quiet longer than `idle_timeout`
//! gets `shutdown_tenant` (`pause`, RAM frozen, row + IP kept, Asleep mark).
//! Wake then unpauses (same IP, RAM intact) instead of recreating.

use std::time::Duration;

use super::AppState;

/// Shared activity handle (re-exported so proxy + server use one type).
pub type ActivityMap = proxy::routing::ActivityMap;

/// Sleep one tenant's container (`pause`) and clear activity tracking.
/// Idempotent: missing container/row is success, not error. The networks
/// row + IP is ALWAYS kept — tenant IPs are stable for life.
pub async fn shutdown_tenant(state: &AppState, tenant_id: &str) -> anyhow::Result<()> {
    if let Some(runtime) = &state.runtime {
        match runtime.snapshot_tenant(tenant_id).await {
            Ok(true) => {}
            Ok(false) => {}
            Err(e) => {
                state
                    .telemetry
                    .builder()
                    .level(telemetry::Level::Warn)
                    .kind("vm.sleep.pause_failed")
                    .category("vm")
                    .tenant_id(Some(tenant_id.to_string()))
                    .message(format!("pause failed: {e}"))
                    .emit();
            }
        }
    }

    // Mark Asleep with a targeted write (never a full-row write from a
    // possibly stale read-back: lost-update hazard).
    let conn = state.gdb.conn()?;
    let _ = db::global::tenants::set_status(
        &conn,
        tenant_id,
        shared::TenantStatus::Asleep.as_str(),
    )
    .await;
    state.activity.remove(tenant_id);

    state
        .telemetry
        .builder()
        .level(telemetry::Level::Info)
        .kind("vm.sleep.completed")
        .category("vm")
        .tenant_id(Some(tenant_id.to_string()))
        .message(format!("container asleep for tenant {tenant_id}"))
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
                if last.elapsed() >= idle_timeout
                    && !tenant_asleep(&state, &tenant_id).await
                {
                    let _ = shutdown_tenant(&state, &tenant_id).await;
                }
            }
            // Also reap reservations with NO recorded activity at all
            // (VMs booted before this process started): treat the
            // reservation as active once so they get a full timeout.
            // Tenants already marked Asleep are skipped — they are done,
            // re-touching them would re-queue them for a second shutdown.
            if let Ok(conn) = state.gdb.conn() {
                if let Ok(rows) = db::global::networks::all(&conn).await {
                    for row in rows {
                        if state.activity.last(&row.tenant_id).is_none()
                            && !tenant_asleep(&state, &row.tenant_id).await
                        {
                            state.activity.touch(&row.tenant_id);
                        }
                    }
                }
            }
        }
    });
}

/// True when the tenant row is marked Asleep (sleeping or sleepless-dead).
async fn tenant_asleep(state: &AppState, tenant_id: &str) -> bool {
    let Ok(conn) = state.gdb.conn() else {
        return false;
    };
    match db::global::tenants::get_by_id(&conn, tenant_id).await {
        Ok(Some(t)) => t.status.as_str() == shared::TenantStatus::Asleep.as_str(),
        _ => false,
    }
}
