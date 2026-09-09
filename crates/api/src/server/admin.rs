//! Admin `/api/admin/*` handlers: staff bootstrap/login, tenants, templates,
//! categories, fleet (VMs), host disks.
//!
//! Every handler here (except bootstrap + staff login) requires a staff
//! Bearer token with role `admin`. Bootstrap is one-time: it 403s as soon as
//! any staff row exists, so frontend onboarding is: bootstrap -> login ->
//! create categories/templates -> create tenants.

use shared::{Category, CategoryStatus, Tenant, TenantStatus, VmTemplate, VmTemplateStatus};

use super::auth_ctx;
use super::idle;
use super::json;
use super::{ApiResponse, AppState};

fn str_field(body: &serde_json::Value, name: &str) -> Option<String> {
    body.get(name)?.as_str().map(str::to_string)
}

fn parse_tenant_status(s: &str) -> TenantStatus {
    match s {
        "suspended" => TenantStatus::Suspended,
        "booting" => TenantStatus::Booting,
        "ready" => TenantStatus::Ready,
        "asleep" => TenantStatus::Asleep,
        "deleted" => TenantStatus::Deleted,
        _ => TenantStatus::Active,
    }
}

// ---------------------------------------------------------------- bootstrap
/// POST /api/admin/bootstrap {username, password} -> 201 {staff, token}.
/// One-time: 403 once ANY staff row exists.
pub async fn bootstrap(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let username = str_field(&body, "username").unwrap_or_default();
    let password = str_field(&body, "password").unwrap_or_default();
    if username.trim().len() < 3 || password.len() < 8 {
        return json::bad_request("username >= 3 chars, password >= 8 chars");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::staff::list(&conn).await {
        Ok(l) if !l.is_empty() => {
            return json::forbidden("already bootstrapped (use staff login)");
        }
        Err(_) => return json::internal(),
        _ => {}
    }
    let password_hash = match auth::login::password::hash_password(&password) {
        Ok(h) => h,
        Err(_) => return json::internal(),
    };
    let staff = db::global::StaffRow {
        id: uuid::Uuid::new_v4().to_string(),
        username,
        password_hash,
        role: "admin".to_string(),
        created_at: shared::now_rfc3339(),
    };
    if db::global::staff::create(&conn, &staff).await.is_err() {
        return json::internal();
    }
    let token = match auth_ctx::mint_session(state, &format!("staff:{}", staff.id)).await {
        Ok(t) => t,
        Err(e) => return e,
    };
    json::created(serde_json::json!({
        "staff": {"id": staff.id, "username": staff.username, "role": staff.role},
        "token": token,
    }))
}

// --------------------------------------------------------------- staff auth
/// POST /api/admin/login {username, password} -> 200 {token, staff}.
pub async fn staff_login(
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
    let staff = match db::global::staff::get_by_username(&conn, &username).await {
        Ok(s) => s,
        Err(_) => return json::internal(),
    };
    let staff = match staff {
        Some(s) => s,
        None => return json::unauthorized(),
    };
    let ok = auth::login::password::verify_password(&password, &staff.password_hash)
        .unwrap_or(false);
    if !ok {
        return json::unauthorized();
    }
    let token = match auth_ctx::mint_session(state, &format!("staff:{}", staff.id)).await {
        Ok(t) => t,
        Err(e) => return e,
    };
    json::ok(serde_json::json!({
        "token": token,
        "staff": {"id": staff.id, "username": staff.username, "role": staff.role},
    }))
}

/// GET /api/admin/me (staff Bearer) -> {staff}.
pub async fn staff_me(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    match auth_ctx::resolve(state, &req).await {
        Ok(auth_ctx::AuthContext::Staff(s)) => json::ok(serde_json::json!({
            "staff": {"id": s.id, "username": s.username, "role": s.role},
        })),
        Ok(auth_ctx::AuthContext::Tenant(_)) => json::forbidden("staff session required"),
        Err(e) => e,
    }
}

// ------------------------------------------------------------------ tenants
/// GET /api/admin/tenants -> {tenants:[...]} (public views).
pub async fn tenants_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::tenants::list(&conn).await {
        Ok(l) => {
            let v: Vec<_> = l.iter().map(auth_ctx::tenant_public).collect();
            json::ok(serde_json::json!({"tenants": v}))
        }
        Err(_) => json::internal(),
    }
}

/// POST /api/admin/tenants {username,password,display_name?,category_id?}
/// -> 201 {tenant}.
pub async fn tenants_create(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let username = str_field(&body, "username").unwrap_or_default();
    let password = str_field(&body, "password").unwrap_or_default();
    if username.trim().len() < 3 || password.len() < 8 {
        return json::bad_request("username >= 3 chars, password >= 8 chars");
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
                None => return json::bad_request("no categories yet"),
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

/// GET /api/admin/tenants/{id} -> {tenant}.
pub async fn tenants_get(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::tenants::get_by_id(&conn, &id).await {
        Ok(Some(t)) => json::ok(serde_json::json!({"tenant": auth_ctx::tenant_public(&t)})),
        Ok(None) => json::not_found("tenant not found"),
        Err(_) => json::internal(),
    }
}

/// PUT /api/admin/tenants/{id} {display_name?,category_id?,status?}
/// -> 200 {tenant}. Username/password are immutable here (delete+recreate).
pub async fn tenants_update(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let mut tenant = match db::global::tenants::get_by_id(&conn, &id).await {
        Ok(Some(t)) => t,
        Ok(None) => return json::not_found("tenant not found"),
        Err(_) => return json::internal(),
    };
    if let Some(d) = str_field(&body, "display_name") {
        tenant.display_name = Some(d);
    }
    if let Some(c) = str_field(&body, "category_id") {
        tenant.category_id = c;
        tenant.category_changed_at = Some(shared::now_rfc3339());
    }
    // Status goes through a targeted write (never part of the full-row
    // write: the row was read moments ago and may already be stale).
    let status_override = str_field(&body, "status");
    if db::global::tenants::update(&conn, &tenant).await.is_err() {
        return json::internal();
    }
    if let Some(s) = status_override {
        let st = parse_tenant_status(&s);
        if db::global::tenants::set_status(&conn, &id, st.as_str())
            .await
            .is_err()
        {
            return json::internal();
        }
        tenant.status = st;
    }
    json::ok(serde_json::json!({"tenant": auth_ctx::tenant_public(&tenant)}))
}

/// DELETE /api/admin/tenants/{id} -> 200 (also sleeps the VM + frees IP).
pub async fn tenants_delete(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let _ = idle::shutdown_tenant(state, &id).await;
    if let Some(runtime) = &state.runtime {
        let _ = runtime.delete_snapshots(&id);
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::tenants::delete(&conn, &id).await {
        Ok(()) => json::ok(serde_json::json!({"ok": true})),
        Err(_) => json::internal(),
    }
}

/// POST /api/admin/tenants/{id}/suspend | .../unsuspend -> 200 {tenant}.
pub async fn tenants_suspend(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    set_suspended(state, req, true).await
}

pub async fn tenants_unsuspend(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    set_suspended(state, req, false).await
}

async fn set_suspended(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
    suspended: bool,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    // .../tenants/{id}/suspend -> second-to-last segment is the id.
    let id = json::second_last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let mut tenant = match db::global::tenants::get_by_id(&conn, &id).await {
        Ok(Some(t)) => t,
        Ok(None) => return json::not_found("tenant not found"),
        Err(_) => return json::internal(),
    };
    tenant.status = if suspended {
        TenantStatus::Suspended
    } else {
        TenantStatus::Active
    };
    if db::global::tenants::set_status(&conn, &id, tenant.status.as_str())
        .await
        .is_err()
    {
        return json::internal();
    }
    if suspended {
        let _ = idle::shutdown_tenant(state, &id).await;
    }
    json::ok(serde_json::json!({"tenant": auth_ctx::tenant_public(&tenant)}))
}

// ---------------------------------------------------------------- templates
fn template_public(t: &VmTemplate) -> serde_json::Value {
    serde_json::json!({
        "id": t.id, "name": t.name, "description": t.description,
        "rootfs_path": t.rootfs_path, "kernel_path": t.kernel_path,
        "initrd_path": t.initrd_path, "vcpu_count": t.vcpu_count,
        "memory_mb": t.memory_mb, "requires_gpu": t.requires_gpu,
        "status": format!("{:?}", t.status),
        "created_at": t.created_at, "updated_at": t.updated_at,
    })
}

/// GET /api/admin/templates -> {templates:[...]}.
pub async fn templates_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::vm_templates::list(&conn).await {
        Ok(l) => {
            let v: Vec<_> = l.iter().map(template_public).collect();
            json::ok(serde_json::json!({"templates": v}))
        }
        Err(_) => json::internal(),
    }
}

/// POST /api/admin/templates {name,kernel_path,initrd_path,vcpu_count,
/// memory_mb,description?,rootfs_path?,requires_gpu?} -> 201 {template}.
pub async fn templates_create(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let name = str_field(&body, "name").unwrap_or_default();
    let kernel_path = str_field(&body, "kernel_path").unwrap_or_default();
    let initrd_path = str_field(&body, "initrd_path").unwrap_or_default();
    if name.trim().is_empty() || kernel_path.is_empty() || initrd_path.is_empty() {
        return json::bad_request("name, kernel_path and initrd_path are required");
    }
    let num = |k: &str, d: u32| {
        body.get(k)
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32)
            .unwrap_or(d)
    };
    let now = shared::now_rfc3339();
    let t = VmTemplate {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description: str_field(&body, "description"),
        rootfs_path: str_field(&body, "rootfs_path").unwrap_or_default(),
        kernel_path,
        initrd_path,
        vcpu_count: num("vcpu_count", 2),
        memory_mb: num("memory_mb", 512),
        requires_gpu: body
            .get("requires_gpu")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        status: VmTemplateStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    if db::global::vm_templates::insert(&conn, &t).await.is_err() {
        return json::internal();
    }
    json::created(serde_json::json!({"template": template_public(&t)}))
}

/// GET /api/admin/templates/{id} -> {template}.
pub async fn templates_get(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::vm_templates::get_by_id(&conn, &id).await {
        Ok(Some(t)) => json::ok(serde_json::json!({"template": template_public(&t)})),
        Ok(None) => json::not_found("template not found"),
        Err(_) => json::internal(),
    }
}

/// PUT /api/admin/templates/{id} {any mutable field} -> 200 {template}.
pub async fn templates_update(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let mut t = match db::global::vm_templates::get_by_id(&conn, &id).await {
        Ok(Some(t)) => t,
        Ok(None) => return json::not_found("template not found"),
        Err(_) => return json::internal(),
    };
    if let Some(v) = str_field(&body, "name") {
        t.name = v;
    }
    if let Some(v) = str_field(&body, "description") {
        t.description = Some(v);
    }
    if let Some(v) = str_field(&body, "rootfs_path") {
        t.rootfs_path = v;
    }
    if let Some(v) = str_field(&body, "kernel_path") {
        t.kernel_path = v;
    }
    if let Some(v) = str_field(&body, "initrd_path") {
        t.initrd_path = v;
    }
    if let Some(v) = body.get("vcpu_count").and_then(serde_json::Value::as_u64) {
        t.vcpu_count = v as u32;
    }
    if let Some(v) = body.get("memory_mb").and_then(serde_json::Value::as_u64) {
        t.memory_mb = v as u32;
    }
    if let Some(v) = body.get("requires_gpu").and_then(serde_json::Value::as_bool) {
        t.requires_gpu = v;
    }
    if let Some(v) = str_field(&body, "status") {
        t.status = if v == "retired" {
            VmTemplateStatus::Retired
        } else {
            VmTemplateStatus::Active
        };
    }
    t.updated_at = shared::now_rfc3339();
    if db::global::vm_templates::update(&conn, &t).await.is_err() {
        return json::internal();
    }
    json::ok(serde_json::json!({"template": template_public(&t)}))
}

/// DELETE /api/admin/templates/{id} -> 200.
pub async fn templates_delete(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::vm_templates::delete(&conn, &id).await {
        Ok(()) => json::ok(serde_json::json!({"ok": true})),
        Err(_) => json::internal(),
    }
}

// --------------------------------------------------------------- categories
fn category_public(c: &Category) -> serde_json::Value {
    serde_json::json!({
        "id": c.id, "name": c.name, "description": c.description,
        "vm_template_id": c.vm_template_id, "max_tenants": c.max_tenants,
        "status": format!("{:?}", c.status),
        "created_at": c.created_at, "updated_at": c.updated_at,
    })
}

/// GET /api/admin/categories -> {categories:[...]}.
pub async fn categories_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::categories::list(&conn).await {
        Ok(l) => {
            let v: Vec<_> = l.iter().map(category_public).collect();
            json::ok(serde_json::json!({"categories": v}))
        }
        Err(_) => json::internal(),
    }
}

/// POST /api/admin/categories {name,vm_template_id,description?,max_tenants?}
/// -> 201 {category}.
pub async fn categories_create(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let name = str_field(&body, "name").unwrap_or_default();
    let vm_template_id = str_field(&body, "vm_template_id").unwrap_or_default();
    if name.trim().is_empty() || vm_template_id.is_empty() {
        return json::bad_request("name and vm_template_id are required");
    }
    let now = shared::now_rfc3339();
    let c = Category {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description: str_field(&body, "description"),
        vm_template_id,
        max_tenants: body
            .get("max_tenants")
            .and_then(serde_json::Value::as_i64),
        status: CategoryStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    if db::global::categories::insert(&conn, &c).await.is_err() {
        return json::internal();
    }
    json::created(serde_json::json!({"category": category_public(&c)}))
}

/// GET /api/admin/categories/{id} -> {category}.
pub async fn categories_get(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::categories::get_by_id(&conn, &id).await {
        Ok(Some(c)) => json::ok(serde_json::json!({"category": category_public(&c)})),
        Ok(None) => json::not_found("category not found"),
        Err(_) => json::internal(),
    }
}

/// PUT /api/admin/categories/{id} -> 200 {category}.
pub async fn categories_update(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let mut c = match db::global::categories::get_by_id(&conn, &id).await {
        Ok(Some(c)) => c,
        Ok(None) => return json::not_found("category not found"),
        Err(_) => return json::internal(),
    };
    if let Some(v) = str_field(&body, "name") {
        c.name = v;
    }
    if let Some(v) = str_field(&body, "description") {
        c.description = Some(v);
    }
    if let Some(v) = str_field(&body, "vm_template_id") {
        c.vm_template_id = v;
    }
    if let Some(v) = body.get("max_tenants").and_then(serde_json::Value::as_i64) {
        c.max_tenants = Some(v);
    }
    if let Some(v) = str_field(&body, "status") {
        c.status = if v == "inactive" {
            CategoryStatus::Inactive
        } else {
            CategoryStatus::Active
        };
    }
    c.updated_at = shared::now_rfc3339();
    if db::global::categories::update(&conn, &c).await.is_err() {
        return json::internal();
    }
    json::ok(serde_json::json!({"category": category_public(&c)}))
}

/// DELETE /api/admin/categories/{id} -> 200.
pub async fn categories_delete(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::categories::delete(&conn, &id).await {
        Ok(()) => json::ok(serde_json::json!({"ok": true})),
        Err(_) => json::internal(),
    }
}

// --------------------------------------------------------------------- fleet
/// GET /api/admin/vms -> {vms:[{tenant_id,username,ip,tap,status}]}.
pub async fn vms_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let rows = match db::global::networks::all(&conn).await {
        Ok(r) => r,
        Err(_) => return json::internal(),
    };
    let tenants = db::global::tenants::list(&conn).await.unwrap_or_default();
    let vms: Vec<_> = rows
        .iter()
        .map(|r| {
            let t = tenants.iter().find(|t| t.id == r.tenant_id);
            serde_json::json!({
                "tenant_id": r.tenant_id,
                "username": t.map(|t| t.username.clone()),
                "ip": r.ip_address,
                "tap": r.tap_device,
                "status": t.map(|t| t.status.as_str()),
            })
        })
        .collect();
    json::ok(serde_json::json!({"vms": vms}))
}

/// POST /api/admin/vms/{tenant_id}/shutdown -> 200 (frees IP; next request
/// re-wakes).
pub async fn vms_shutdown(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = json::second_last_seg(&req);
    match idle::shutdown_tenant(state, &tenant_id).await {
        Ok(()) => json::ok(serde_json::json!({"ok": true, "status": "asleep"})),
        Err(_) => json::internal(),
    }
}

// --------------------------------------------------------------------- disks
/// GET /api/admin/disks -> {disks:[{name,path,size_bytes}]}: guest images
/// (kernels, initramfs) present on the host. Admin picks these paths when
/// creating templates.
pub async fn disks_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let dir = state.disks_dir.clone();
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|_| json::internal());
    let entries = match entries {
        Ok(e) => e,
        Err(e) => return e,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let is_image = name.ends_with(".bin")
            || name.ends_with(".gz")
            || name.ends_with(".cpio")
            || name.ends_with(".ext4")
            || name.ends_with(".img");
        if !is_image || !path.is_file() {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(serde_json::json!({
            "name": name,
            "path": path.to_string_lossy(),
            "size_bytes": size,
        }));
    }
    out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    json::ok(serde_json::json!({"disks": out, "dir": dir}))
}
