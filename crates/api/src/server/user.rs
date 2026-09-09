//! User-facing `/api` handlers: signup, login (with VM pre-warm), logout,
//! self-service (`/api/me`, VM wake/status).
//!
//! Auth is DB-backed (tenants.password_hash via Argon-style hash in the
//! `auth` crate, sessions in `sessions` table, 24h expiry) — restart-safe,
//! unlike the in-memory auth stub.

use shared::{Tenant, TenantStatus};

use super::auth_ctx::{self, AuthContext};
use super::json;
use super::{ApiResponse, AppState};

fn str_field(body: &serde_json::Value, name: &str) -> Option<String> {
    body.get(name)?.as_str().map(str::to_string)
}

/// POST /api/auth/signup {username, password, display_name?, category_id?}
/// -> 201 {tenant}. First category is used when category_id is omitted.
pub async fn signup(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let username = str_field(&body, "username").unwrap_or_default();
    let password = str_field(&body, "password").unwrap_or_default();
    if username.trim().len() < 3 {
        return json::bad_request("username must be >= 3 chars");
    }
    if password.len() < 8 {
        return json::bad_request("password must be >= 8 chars");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let existing = match db::global::tenants::list(&conn).await {
        Ok(l) => l,
        Err(_) => return json::internal(),
    };
    if existing.iter().any(|t| t.username == username) {
        return json::conflict("username taken");
    }
    let category_id = match str_field(&body, "category_id") {
        Some(c) => c,
        None => match db::global::categories::list(&conn).await {
            Ok(cats) => match cats.first() {
                Some(c) => c.id.clone(),
                None => return json::bad_request("no categories yet (admin must create one)"),
            },
            Err(_) => return json::internal(),
        },
    };
    let password_hash = match auth::login::password::hash_password(&password) {
        Ok(h) => h,
        Err(_) => return json::internal(),
    };
    let now = shared::now_rfc3339();
    let tenant = Tenant {
        id: uuid::Uuid::new_v4().to_string(),
        username,
        password_hash,
        display_name: str_field(&body, "display_name"),
        category_id,
        vm_ip: None,
        vm_process_id: None,
        vm_template_id: None,
        tap_device: None,
        status: TenantStatus::Active,
        db_path: String::new(),
        created_at: now,
        last_login: None,
        category_changed_at: None,
    };
    if db::global::tenants::create(&conn, &tenant, &state.telemetry)
        .await
        .is_err()
    {
        return json::internal();
    }
    json::created(serde_json::json!({"tenant": auth_ctx::tenant_public(&tenant)}))
}

/// POST /api/auth/login {username, password} -> 200 {token, tenant}.
/// Pre-warms the tenant's VM in the background so the first request is fast.
pub async fn login(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let username = str_field(&body, "username").unwrap_or_default();
    let password = str_field(&body, "password").unwrap_or_default();
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let tenants = match db::global::tenants::list(&conn).await {
        Ok(l) => l,
        Err(_) => return json::internal(),
    };
    let tenant = tenants.iter().find(|t| t.username == username);
    let tenant = match tenant {
        Some(t) => t.clone(),
        None => return json::unauthorized(),
    };
    let ok = auth::login::password::verify_password(&password, &tenant.password_hash)
        .unwrap_or(false);
    if !ok {
        return json::unauthorized();
    }
    if tenant.status.as_str() == TenantStatus::Suspended.as_str() {
        return json::forbidden("account suspended");
    }
    let token = match auth_ctx::mint_session(state, &tenant.id).await {
        Ok(t) => t,
        Err(e) => return e,
    };
    // Pre-warm (Phase 4 login->VM): boot in background, never block login.
    if let Some(runtime) = state.runtime.clone() {
        let tid = tenant.id.clone();
        let activity = state.activity.clone();
        tokio::spawn(async move {
            activity.touch(&tid);
            let _ = runtime.ensure_running(&tid).await;
        });
    }
    json::ok(serde_json::json!({
        "token": token,
        "tenant": auth_ctx::tenant_public(&tenant),
        "apps_url": state.apps_url(&tenant.id),
    }))
}

/// POST /api/auth/logout (Bearer) -> 200.
pub async fn logout(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let token = match json::bearer(&req) {
        Some(t) => t,
        None => return json::unauthorized(),
    };
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let _ = db::global::sessions::delete(&conn, &token).await;
    json::ok(serde_json::json!({"ok": true}))
}

/// GET /api/me (Bearer) -> {tenant, vm:{status,ip}, apps_url}.
pub async fn me(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let tenant = match auth_ctx::resolve(state, &req).await {
        Ok(AuthContext::Tenant(t)) => t,
        Ok(AuthContext::Staff(_)) => {
            return json::forbidden("tenant session required");
        }
        Err(e) => return e,
    };
    let (status, ip) = vm_view(state, &tenant.id).await;
    json::ok(serde_json::json!({
        "tenant": auth_ctx::tenant_public(&tenant),
        "vm": {"status": status, "ip": ip},
        "apps_url": state.apps_url(&tenant.id),
    }))
}

/// GET /api/me/vm/status (Bearer) -> {status, ip}.
pub async fn vm_status(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let tenant = match auth_ctx::resolve(state, &req).await {
        Ok(AuthContext::Tenant(t)) => t,
        Ok(AuthContext::Staff(_)) => {
            return json::forbidden("tenant session required");
        }
        Err(e) => return e,
    };
    let (status, ip) = vm_view(state, &tenant.id).await;
    json::ok(serde_json::json!({"status": status, "ip": ip}))
}

/// POST /api/me/vm/wake (Bearer) -> 202 {status, ip}. Boots when asleep.
pub async fn vm_wake(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let tenant = match auth_ctx::resolve(state, &req).await {
        Ok(AuthContext::Tenant(t)) => t,
        Ok(AuthContext::Staff(_)) => {
            return json::forbidden("tenant session required");
        }
        Err(e) => return e,
    };
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    state.activity.touch(&tenant.id);
    match runtime.ensure_running(&tenant.id).await {
        Ok(addr) => {
            let ip = addr.split(':').next().unwrap_or("").to_string();
            json::accepted(serde_json::json!({"status": "running", "ip": ip}))
        }
        Err(_) => json::unavailable("vm boot failed"),
    }
}

/// VM view from the networks reservation (None row = asleep).
async fn vm_view(state: &AppState, tenant_id: &str) -> (String, Option<String>) {
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return ("unknown".to_string(), None),
    };
    let rows = match db::global::networks::all(&conn).await {
        Ok(r) => r,
        Err(_) => return ("unknown".to_string(), None),
    };
    match rows.into_iter().find(|r| r.tenant_id == tenant_id) {
        Some(r) => ("running".to_string(), Some(r.ip_address)),
        None => ("asleep".to_string(), None),
    }
}
