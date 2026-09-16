//! Admin `/api/admin/*` handlers: staff bootstrap/login, tenants, templates,
//! categories, fleet (VMs), host disks.
//!
//! Every handler here (except bootstrap + staff login) requires a staff
//! Bearer token with role `admin`. Bootstrap is one-time: it 403s as soon as
//! any staff row exists, so frontend onboarding is: bootstrap -> login ->
//! create categories/templates -> create tenants.

use http_body_util::Full;
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

/// GET /api/admin/categories/{id}/tools -> {tools:[dotted ops], restricted:bool}.
/// Empty tools + restricted=false means the category may list every tool.
pub async fn categories_tools_get(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    // /api/admin/categories/{id}/tools -> {id} is second-to-last.
    let id = json::second_last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    if db::global::categories::get_by_id(&conn, &id)
        .await
        .map(|c| c.is_none())
        .unwrap_or(true)
    {
        return json::not_found("category not found");
    }
    match db::global::category_tools::list(&conn, &id).await {
        Ok(tools) => json::ok(
            serde_json::json!({"tools": tools, "restricted": !tools.is_empty()}),
        ),
        Err(_) => json::internal(),
    }
}

/// PUT /api/admin/categories/{id}/tools {tools:[dotted ops]} -> {tools}.
/// Replaces the grant set; [] (or missing) clears the restriction.
/// Unknown op names are rejected.
pub async fn categories_tools_set(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    // /api/admin/categories/{id}/tools -> {id} is second-to-last.
    let id = json::second_last_seg(&req);
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let tools: Vec<String> = body
        .get("tools")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    for t in &tools {
        if !supervisor_server::mcp::tools::is_known_tool(t) {
            return json::bad_request(format!("unknown tool {t}"));
        }
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    if db::global::categories::get_by_id(&conn, &id)
        .await
        .map(|c| c.is_none())
        .unwrap_or(true)
    {
        return json::not_found("category not found");
    }
    match db::global::category_tools::set_all(&conn, &id, &tools).await {
        Ok(()) => json::ok(serde_json::json!({"tools": tools})),
        Err(_) => json::internal(),
    }
}

/// GET /api/admin/logs?tenant=&level=&kind=&q=&since=&limit=
/// Tail the host telemetry daily JSON files newest-first, filtered.
/// `since` is an RFC3339 instant (or unix seconds); `limit` caps at 500.
pub async fn logs_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let q = req.uri().query().unwrap_or("").to_string();
    let params: std::collections::HashMap<&str, &str> = q
        .split('&')
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            Some((it.next()?, it.next().unwrap_or("")))
        })
        .collect();
    let tenant = params.get("tenant").map(|s| s.to_string()).unwrap_or_default();
    let level = params.get("level").map(|s| s.to_string()).unwrap_or_default();
    let kind = params.get("kind").map(|s| s.to_string()).unwrap_or_default();
    let query = params.get("q").map(|s| s.to_string()).unwrap_or_default();
    let limit: usize = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(200)
        .clamp(1, 500);
    let since_secs: Option<i64> = params.get("since").and_then(|v| {
        v.parse::<i64>().ok().or_else(|| {
            chrono::DateTime::parse_from_rfc3339(v)
                .ok()
                .map(|d| d.timestamp())
        })
    });
    let mut files: Vec<_> = std::fs::read_dir(&state.telemetry_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str()) == Some("json")
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files.reverse();
    let mut out = Vec::new();
    'files: for path in files {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let mut lines: Vec<&str> = text.lines().collect();
        lines.reverse();
        for line in lines {
            if out.len() >= limit {
                break 'files;
            }
            let v: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("");
            if !tenant.is_empty()
                && v.get("tenant_id").and_then(|x| x.as_str()).unwrap_or("")
                    != tenant.as_str()
                && !s("message").contains(tenant.as_str())
            {
                continue;
            }
            if !level.is_empty() && !s("level").eq_ignore_ascii_case(&level) {
                continue;
            }
            if !kind.is_empty() && !(s("type").contains(&kind) || s("kind").contains(&kind)) {
                continue;
            }
            if !query.is_empty() && !line.contains(&query) {
                continue;
            }
            if let Some(since) = since_secs {
                let ts = v
                    .get("timestamp")
                    .and_then(|t| t.as_str())
                    .and_then(|t| {
                        chrono::DateTime::parse_from_rfc3339(t)
                            .ok()
                            .map(|d| d.timestamp())
                    })
                    .unwrap_or(0);
                if ts < since {
                    continue;
                }
            }
            out.push(v);
        }
    }
    json::ok(serde_json::json!({"logs": out}))
}

/// GET /api/admin/categories/{id}/configs -> {configs:[...]} (no blob bytes,
/// content preview only).
pub async fn category_configs_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::second_last_seg(&req);
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::deployment_configs::list(&conn).await {
        Ok(all) => {
            let v: Vec<_> = all
                .into_iter()
                .filter(|d| d.category_id == id)
                .map(|d| {
                    serde_json::json!({
                        "id": d.id, "category_id": d.category_id,
                        "config_type": d.config_type, "name": d.name,
                        "path": d.path,
                        "content_preview": d.content
                            .as_ref()
                            .map(|b| String::from_utf8_lossy(&b[..b.len().min(500)]).to_string()),
                        "checksum": d.checksum, "priority": d.priority,
                        "created_at": d.created_at, "updated_at": d.updated_at,
                    })
                })
                .collect();
            json::ok(serde_json::json!({"configs": v}))
        }
        Err(_) => json::internal(),
    }
}

/// POST /api/admin/categories/{id}/configs {config_type,name,path,content?,priority?}
/// -> 201 {config} (content stored as UTF-8 bytes).
pub async fn category_configs_create(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let id = json::second_last_seg(&req);
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let name = str_field(&body, "name").unwrap_or_default();
    let path = str_field(&body, "path").unwrap_or_default();
    if name.trim().is_empty() || path.trim().is_empty() {
        return json::bad_request("name and path are required");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    if db::global::categories::get_by_id(&conn, &id)
        .await
        .map(|c| c.is_none())
        .unwrap_or(true)
    {
        return json::not_found("category not found");
    }
    let now = shared::now_rfc3339();
    let d = shared::DeploymentConfig {
        id: uuid::Uuid::new_v4().to_string(),
        category_id: id,
        config_type: str_field(&body, "config_type").unwrap_or_else(|| "file".to_string()),
        name,
        path,
        content: body
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.as_bytes().to_vec()),
        checksum: None,
        priority: body
            .get("priority")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32,
        created_at: now.clone(),
        updated_at: now,
    };
    match db::global::deployment_configs::insert(&conn, &d).await {
        Ok(()) => json::created(serde_json::json!({"config": {
            "id": d.id, "category_id": d.category_id, "config_type": d.config_type,
            "name": d.name, "path": d.path, "priority": d.priority,
        }})),
        Err(_) => json::internal(),
    }
}

/// DELETE /api/admin/categories/{id}/configs/{config_id} -> {ok}.
pub async fn category_configs_delete(
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
    match db::global::deployment_configs::delete(&conn, &id).await {
        Ok(()) => json::ok(serde_json::json!({"ok": true})),
        Err(_) => json::internal(),
    }
}

/// GET /api/admin/dashboard?tenant={id|all} -> snapshot JSON.
/// Same shape as the WS stream frames, over plain REST: used by the admin
/// UI as a fallback when websockets are blocked, and as a curl-debuggable
/// probe (`?tenant=all` returns {tenants:[...]}).
pub async fn dashboard_snapshot(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|pair| {
            let mut it = pair.splitn(2, '=');
            (it.next() == Some("tenant")).then(|| it.next().unwrap_or("").to_string())
        })
        .unwrap_or_default();
    if tenant.trim().is_empty() {
        return json::bad_request("missing ?tenant=<id|all>");
    }
    if tenant == "all" {
        let ids: Vec<String> = match state.gdb.conn() {
            Ok(c) => db::global::tenants::list(&c)
                .await
                .map(|rows| rows.into_iter().map(|t| t.id).collect())
                .unwrap_or_default(),
            Err(_) => return json::internal(),
        };
        let mut snaps = Vec::new();
        for id in ids {
            snaps.push(super::stream::snapshot(state, &id).await);
        }
        return json::ok(serde_json::json!({"tenants": snaps}));
    }
    json::ok(super::stream::snapshot(state, &tenant).await)
}

/// GET /api/admin/tools -> {tools:[{name, family, description}]}.
/// Static catalog of every in-VM supervisor tool an admin may grant.
pub async fn tools_catalog(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    json::ok(serde_json::json!({"tools": tool_catalog()}))
}

fn tool_catalog() -> Vec<serde_json::Value> {
    const C: &[(&str, &str, &str)] = &[
        ("portfolio", "portfolio.show", "Show portfolio summary"),
        ("portfolio", "portfolio.update", "Update portfolio profile"),
        ("portfolio", "portfolio.create_goal", "Create a goal"),
        ("portfolio", "portfolio.update_goal", "Update a goal"),
        ("portfolio", "portfolio.list_goals", "List goals"),
        ("portfolio", "portfolio.get_goal", "Get a goal"),
        ("portfolio", "portfolio.add_milestone", "Add milestone"),
        ("portfolio", "portfolio.list_milestones", "List milestones"),
        ("portfolio", "portfolio.update_milestone", "Update milestone"),
        ("portfolio", "portfolio.create_task", "Create task"),
        ("portfolio", "portfolio.update_task", "Update task"),
        ("portfolio", "portfolio.list_tasks", "List tasks"),
        ("portfolio", "portfolio.get_task", "Get task"),
        ("portfolio", "portfolio.submit_attempt", "Submit attempt"),
        ("portfolio", "portfolio.resolve_attempt", "Resolve attempt"),
        ("portfolio", "portfolio.list_attempts", "List attempts"),
        ("portfolio", "portfolio.get_attempt", "Get attempt"),
        ("portfolio", "portfolio.record_skill", "Record skill"),
        ("portfolio", "portfolio.update_skill", "Update skill"),
        ("portfolio", "portfolio.list_skills", "List skills"),
        ("portfolio", "portfolio.get_skill", "Get skill"),
        ("portfolio", "portfolio.award_achievement", "Award achievement"),
        ("portfolio", "portfolio.list_achievements", "List achievements"),
        ("portfolio", "portfolio.get_achievement", "Get achievement"),
        ("portfolio", "portfolio.generate_report", "Generate report"),
        ("portfolio", "portfolio.list_reports", "List reports"),
        ("portfolio", "portfolio.get_report", "Get report"),
        ("portfolio", "portfolio.list_timeline", "List timeline"),
        ("portfolio", "portfolio.get_timeline", "Get timeline entry"),
        ("portfolio", "portfolio.add_timeline", "Add timeline note"),
    ];
    C.iter()
        .map(|(family, name, description)| {
            serde_json::json!({"family": family, "name": name, "description": description})
        })
        .collect()
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
/// re-wakes). Legacy alias of `sleep`.
pub async fn vms_shutdown(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    vms_sleep(state, req).await
}

/// POST /api/admin/vms/{tenant_id}/sleep -> {ok, status}.
/// SLEEP: snapshot RAM first, then verified kill. Next wake RESTORES fast
/// (same IP, RAM preserved). Use before image changes that keep the boot
/// configuration, or to park an idle tenant.
pub async fn vms_sleep(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = json::second_last_seg(&req);
    match idle::shutdown_tenant(state, &tenant_id).await {
        Ok(()) => json::ok(serde_json::json!({"ok": true, "status": "asleep", "mode": "sleep"})),
        Err(_) => json::internal(),
    }
}

/// POST /api/admin/vms/{tenant_id}/stop -> {ok, status}.
/// STOP: verified kill only — no new snapshot, existing snapshots kept.
/// Next wake restores if a snapshot exists, else fresh-boots. The VM is
/// simply off; nothing is wiped.
pub async fn vms_stop(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = json::second_last_seg(&req);
    let sock_pattern = format!("/tmp/fc_{tenant_id}.sock");
    vm::runtime::kill_process_pattern(&sock_pattern).await;
    state.activity.remove(&tenant_id);
    json::ok(serde_json::json!({"ok": true, "status": "asleep", "mode": "stop"}))
}

/// POST /api/admin/vms/{tenant_id}/kill -> {ok, status}.
/// KILL: verified kill + snapshots deleted. IP row, tap and DATA DISK are
/// KEPT — only RAM state goes. Next wake FRESH-BOOTS on the same IP (new
/// image, disk prompt re-seeded). Use after image changes, boot-config
/// changes, or a wedged guest. This is the button that guarantees the VM
/// starts from down, never from a snapshot.
pub async fn vms_kill(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = json::second_last_seg(&req);
    let sock_pattern = format!("/tmp/fc_{tenant_id}.sock");
    vm::runtime::kill_process_pattern(&sock_pattern).await;
    if let Some(runtime) = &state.runtime {
        let _ = runtime.delete_snapshots(&tenant_id);
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let _ = db::global::tenants::set_status(
        &conn,
        &tenant_id,
        shared::TenantStatus::Asleep.as_str(),
    )
    .await;
    state.activity.remove(&tenant_id);
    json::ok(serde_json::json!({"ok": true, "status": "asleep", "mode": "kill"}))
}

// ------------------------------------------------------- tenant data disk
/// GET /api/admin/tenants/{id}/disk -> {exists,size_bytes,size_gb,used_bytes}:
/// the tenant's persistent data-disk image (`<disks>/<tenant>.img`).
/// `used_bytes` is sparse-aware host usage (null when unknown).
pub async fn tenant_disk_show(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = json::second_last_seg(&req);
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    let size = runtime.disk_size_bytes(&tenant_id);
    let used = runtime.disk_used_bytes(&tenant_id).await;
    json::ok(serde_json::json!({
        "tenant_id": tenant_id,
        "exists": size.is_some(),
        "size_bytes": size,
        "size_gb": size.map(|b| (b as f64) / 1024.0 / 1024.0 / 1024.0),
        "used_bytes": used,
    }))
}

/// POST /api/admin/tenants/{id}/disk/resize {size_gb} -> grow offline.
///
/// Grow-only, capped at 64 GiB. The VM is killed first when live (offline
/// resize is the only corruption-safe kind: a live guest caches filesystem
/// metadata). Disk bytes are never at risk (`truncate` appends, `e2fsck`
/// then `resize2fs`); in-flight RAM state is not preserved — same as any
/// reboot. Sleep snapshots are invalidated by the geometry change, so a
/// re-wake fresh-boots (disk data intact). Re-wakes automatically when the
/// VM was live; otherwise it stays asleep until next use.
pub async fn tenant_disk_resize(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    // Route: /api/admin/tenants/{id}/disk/resize -> id is 3rd from last.
    let tenant_id = json::nth_from_last_seg(&req, 3);
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let size_gb = match body.get("size_gb").and_then(|v| v.as_u64()) {
        Some(n) => n,
        None => return json::bad_request("size_gb required (integer GiB, grow-only)"),
    };
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    let was_live = vm::runtime::fc_socket_path(&tenant_id).exists();
    if was_live {
        runtime.kill_vm_process(&tenant_id).await;
    }
    match runtime.resize_data_disk(&tenant_id, size_gb).await {
        Ok(now_bytes) => {
            let ip = if was_live {
                state.activity.touch(&tenant_id);
                runtime
                    .ensure_running(&tenant_id)
                    .await
                    .ok()
                    .and_then(|a| a.split(':').next().map(str::to_string))
            } else {
                None
            };
            json::ok(serde_json::json!({
                "tenant_id": tenant_id,
                "size_gb": size_gb,
                "size_bytes": now_bytes,
                "re_waked": ip.is_some(),
                "ip": ip,
            }))
        }
        Err(e) => {
            // Restore service when we took it down: best-effort re-wake so
            // a failed resize never strands a tenant dark.
            if was_live {
                state.activity.touch(&tenant_id);
                let _ = runtime.ensure_running(&tenant_id).await;
            }
            json::json(
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({"error": "disk resize failed", "detail": e.to_string()}),
            )
        }
    }
}

/// POST /api/admin/workspace/sync {tenant_id} -> {written, re_waked}.
/// Pushes the resolved agent system prompt (SYSTEM.md) into the tenant's
/// data disk via offline loop mount (`.bak-date` backup). Kills the VM
/// first when live, re-wakes when it was live so zw picks the prompt up.
/// Works for SLEEPING VMs too (disk image on host, no VM needed).
pub async fn workspace_sync(
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
    let tenant_id = match body.get("tenant_id").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return json::bad_request("tenant_id required"),
    };
    let force = body.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    let was_live = vm::runtime::fc_socket_path(&tenant_id).exists();
    if was_live {
        runtime.kill_vm_process(&tenant_id).await;
    }
    match runtime.sync_workspace_seed(&tenant_id, force).await {
        Ok(written) => {
            let ip = if was_live {
                state.activity.touch(&tenant_id);
                runtime
                    .ensure_running(&tenant_id)
                    .await
                    .ok()
                    .and_then(|a| a.split(':').next().map(str::to_string))
            } else {
                None
            };
            json::ok(serde_json::json!({
                "tenant_id": tenant_id,
                "written": written,
                "force": force,
                "re_waked": ip.is_some(),
                "ip": ip,
            }))
        }
        Err(e) => {
            if was_live {
                state.activity.touch(&tenant_id);
                let _ = runtime.ensure_running(&tenant_id).await;
            }
            json::json(
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({"error": "workspace sync failed", "detail": e.to_string()}),
            )
        }
    }
}

/// POST /api/admin/workspace/sync-bulk {tenant_ids?[], category_id?, all?}
/// -> {results:[{tenant_id,written,re_waked,ip?,error?}]}.
/// Pushes SYSTEM.md to many tenants. Offline-safe: iterates sequentially
/// (loop mounts contend if parallel). Resolves targets as: tenant_ids >
/// category_id > all. Each tenant handled like workspace_sync (kill when
/// live, write prompt, re-wake when live).
pub async fn workspace_sync_bulk(
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
    let force = body.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let tenant_ids: Vec<String> = if let Some(arr) = body.get("tenant_ids").and_then(|v| v.as_array()) {
        arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
    } else if let Some(cat) = body.get("category_id").and_then(|v| v.as_str()) {
        let conn = match state.gdb.conn() {
            Ok(c) => c,
            Err(_) => return json::internal(),
        };
        match db::global::tenants::list(&conn).await {
            Ok(rows) => rows.into_iter().filter(|t| t.category_id == cat).map(|t| t.id).collect(),
            Err(_) => return json::internal(),
        }
    } else if body.get("all").and_then(|v| v.as_bool()).unwrap_or(false) {
        let conn = match state.gdb.conn() {
            Ok(c) => c,
            Err(_) => return json::internal(),
        };
        match db::global::tenants::list(&conn).await {
            Ok(rows) => rows.into_iter().map(|t| t.id).collect(),
            Err(_) => return json::internal(),
        }
    } else {
        return json::bad_request("provide tenant_ids[] or category_id or all:true");
    };
    if tenant_ids.is_empty() {
        return json::ok(serde_json::json!({"results": [], "force": force}));
    }
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    let mut results = Vec::new();
    for tid in tenant_ids {
        let was_live = vm::runtime::fc_socket_path(&tid).exists();
        if was_live {
            runtime.kill_vm_process(&tid).await;
        }
        match runtime.sync_workspace_seed(&tid, force).await {
            Ok(written) => {
                let ip = if was_live {
                    state.activity.touch(&tid);
                    runtime
                        .ensure_running(&tid)
                        .await
                        .ok()
                        .and_then(|a| a.split(':').next().map(str::to_string))
                } else {
                    None
                };
                results.push(serde_json::json!({
                    "tenant_id": tid, "written": written, "force": force,
                    "re_waked": ip.is_some(), "ip": ip,
                }));
            }
            Err(e) => {
                if was_live {
                    state.activity.touch(&tid);
                    let _ = runtime.ensure_running(&tid).await;
                }
                results.push(serde_json::json!({
                    "tenant_id": tid, "error": e.to_string(),
                }));
            }
        }
    }
    json::ok(serde_json::json!({"results": results, "force": force}))
}

// ------------------------------------------------------- system prompt
fn query_param(req: &hyper::Request<hyper::body::Incoming>, key: &str) -> String {
    req.uri()
        .query()
        .unwrap_or("")
        .split('&')
        .filter_map(|pair| {
            let mut it = pair.splitn(2, '=');
            let k = it.next()?;
            let v = it.next().unwrap_or("");
            (k == key).then(|| v.to_string())
        })
        .next()
        .unwrap_or_default()
}

fn host_prompt_path(state: &AppState) -> std::path::PathBuf {
    match &state.runtime {
        Some(r) => r.host_system_prompt_path(),
        None => vm::runtime::host_system_prompt_path_for(
            &state.system_prompt_host_path,
            &state.data_disks_dir,
        ),
    }
}

/// GET /api/admin/system-prompt?scope=global|category|tenant&ref=<id>
/// -> {scope, ref_id, content, source}. `source` is `db`, `host-file`
/// (global fallback so the editor never opens empty) or `none`.
pub async fn system_prompt_get(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let scope = query_param(&req, "scope");
    let scope = if scope.is_empty() { "global".to_string() } else { scope };
    if !["global", "category", "tenant"].contains(&scope.as_str()) {
        return json::bad_request("scope must be global|category|tenant");
    }
    let ref_id = query_param(&req, "ref");
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::system_prompts::get(&conn, &scope, &ref_id).await {
        Ok(Some(c)) => json::ok(serde_json::json!({
            "scope": scope, "ref_id": ref_id, "content": c, "source": "db",
        })),
        Ok(None) => {
            let (content, source) = if scope == "global" {
                let hp = host_prompt_path(state);
                (
                    std::fs::read_to_string(hp).unwrap_or_default(),
                    "host-file".to_string(),
                )
            } else {
                (String::new(), "none".to_string())
            };
            json::ok(serde_json::json!({
                "scope": scope, "ref_id": ref_id, "content": content, "source": source,
            }))
        }
        Err(_) => json::internal(),
    }
}

/// PUT /api/admin/system-prompt {scope, ref_id?, content} -> {scope, ref_id}.
/// Stores one scope row; distribution to tenant disks happens through the
/// workspace sync endpoints (SYSTEM.md is written on every sync).
pub async fn system_prompt_set(
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
    let scope = body
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("global")
        .to_string();
    if !["global", "category", "tenant"].contains(&scope.as_str()) {
        return json::bad_request("scope must be global|category|tenant");
    }
    let ref_id = body
        .get("ref_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if scope != "global" && ref_id.is_empty() {
        return json::bad_request("ref_id required for category|tenant scope");
    }
    let content = body
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if content.trim().is_empty() {
        return json::bad_request("content must not be empty");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::system_prompts::set(&conn, &scope, &ref_id, &content, &shared::now_rfc3339()).await {
        Ok(()) => json::ok(serde_json::json!({"scope": scope, "ref_id": ref_id})),
        Err(_) => json::internal(),
    }
}

/// GET /api/admin/launchers?scope=global|category|tenant&ref=<id>
/// -> {scope, ref_id, scripts: {zw: content|null, engram: content|null}}.
/// Reads the launcher scripts the supervisor will demand for that scope.
pub async fn launchers_get(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let scope = query_param(&req, "scope");
    let scope = if scope.is_empty() { "global".to_string() } else { scope };
    if !["global", "category", "tenant"].contains(&scope.as_str()) {
        return json::bad_request("scope must be global|category|tenant");
    }
    let ref_id = query_param(&req, "ref");
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let mut scripts = serde_json::Map::new();
    for name in db::global::launchers::NAMES {
        match db::global::launchers::get(&conn, &scope, &ref_id, name).await {
            Ok(c) => {
                scripts.insert(
                    name.to_string(),
                    c.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                );
            }
            Err(_) => return json::internal(),
        }
    }
    json::ok(serde_json::json!({ "scope": scope, "ref_id": ref_id, "scripts": scripts }))
}

/// PUT /api/admin/launchers {scope, ref_id?, name, content}
/// -> {scope, ref_id, name}. Stores one launcher script row; supervisors
/// pick it up on next demand (boot or crash-loop refetch).
pub async fn launchers_set(
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
    let scope = body
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("global")
        .to_string();
    if !["global", "category", "tenant"].contains(&scope.as_str()) {
        return json::bad_request("scope must be global|category|tenant");
    }
    let ref_id = body
        .get("ref_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if scope != "global" && ref_id.is_empty() {
        return json::bad_request("ref_id required for category|tenant scope");
    }
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !db::global::launchers::NAMES.contains(&name.as_str()) {
        return json::bad_request("name must be zw|engram");
    }
    let content = body
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if content.trim().is_empty() {
        return json::bad_request("content must not be empty");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::launchers::set(&conn, &scope, &ref_id, &name, &content, &shared::now_rfc3339()).await {
        Ok(()) => json::ok(serde_json::json!({ "scope": scope, "ref_id": ref_id, "name": name })),
        Err(_) => json::internal(),
    }
}

/// DELETE /api/admin/launchers?scope=&ref=&name= -> {deleted: bool}.
/// Removes a launcher override row (falls back to the next precedence
/// level). Scope/ref from query or JSON body; name must be zw|engram.
pub async fn launchers_delete(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let (mut scope, mut ref_id, mut name) = (
        query_param(&req, "scope"),
        query_param(&req, "ref"),
        query_param(&req, "name"),
    );
    if scope.is_empty() {
        if let Ok(body) = json::read_json(req).await {
            scope = body
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ref_id = body
                .get("ref")
                .and_then(|v| v.as_str())
                .or_else(|| body.get("ref_id").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        }
    }
    if !["global", "category", "tenant"].contains(&scope.as_str()) {
        return json::bad_request("scope must be global|category|tenant");
    }
    if !db::global::launchers::NAMES.contains(&name.as_str()) {
        return json::bad_request("name must be zw|engram");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match conn
        .execute(
            "DELETE FROM launcher_scripts WHERE scope = ?1 AND ref_id = ?2 AND name = ?3",
            (scope.as_str(), ref_id.as_str(), name.as_str()),
        )
        .await
    {
        Ok(n) => json::ok(serde_json::json!({"deleted": n > 0})),
        Err(_) => json::internal(),
    }
}

/// GET /api/supervisor/binaries/manifest (Bearer = non-empty token)
/// -> {files:[{path,sha256,size}]}. Guest supervisors demand binaries with
/// this; kept out of the admin surface (no session in the VM).
pub async fn supervisor_binaries_manifest(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_supervisor(state, &req) {
        return e;
    }
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let files = shared::binaries::manifest_cached(&dir);
    json::ok(serde_json::json!({ "files": files }))
}

/// GET /api/supervisor/binaries/file?path=<rel> (Bearer = wrapper
/// auth_token) -> raw octet-stream. Streams from disk so a 92MB engram
/// never lands in wrapper RAM.
pub async fn supervisor_binaries_file(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_supervisor(state, &req) {
        return e;
    }
    let rel = query_param(&req, "path");
    if !shared::binaries::validate_rel(&rel) {
        return json::bad_request("bad path");
    }
    let full = std::path::PathBuf::from(&state.binaries_dir).join(&rel);
    // Wrapper-side read: the host has RAM to spare, and `ApiResponse` is
    // `Response<Full<Bytes>>` (no streaming body type). The GUEST streams
    // this to disk in 256KB pieces, so a 92MB engram never lands in the VM's
    // 490MB RAM (that was the reboot-loop cause).
    let data = match std::fs::read(&full) {
        Ok(d) => d,
        Err(_) => return json::not_found("binary not found"),
    };
    let mut res = hyper::Response::new(Full::new(hyper::body::Bytes::from(data)));
    *res.status_mut() = hyper::StatusCode::OK;
    res.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/octet-stream"),
    );
    res
}

/// GET /api/admin/binaries -> {dir, files:[{path,sha256,size}]}.
/// Version manifest of the host binaries folder supervisors demand from.
pub async fn binaries_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let files = shared::binaries::manifest_cached(&dir);
    json::ok(serde_json::json!({ "dir": dir.to_string_lossy(), "files": files }))
}

/// PUT /api/admin/binaries/file?path=<rel> with raw octet-stream body
/// (200MB cap) -> {path, sha256, size}. Portal uploads land instantly in
/// the configured binaries folder; VMs pick them up on next demand
/// (boot / crash redemand / Kill). Atomic write (tmp + rename).
pub async fn binaries_upload(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let rel = query_param(&req, "path");
    if !shared::binaries::validate_rel(&rel) {
        return json::bad_request("bad path (a-z 0-9 . _ - /, no .., max 128)");
    }
    use http_body_util::BodyExt;
    let bytes = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => return json::bad_request("unreadable body"),
    };
    const LIMIT: usize = 200 * 1024 * 1024;
    if bytes.len() > LIMIT {
        return json::bad_request("body too large (200MB cap)");
    }
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let dst = dir.join(&rel);
    if let Some(parent) = dst.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return json::internal();
        }
    }
    let tmp = dst.with_extension("upload-tmp");
    if std::fs::write(&tmp, &bytes).is_err() || std::fs::rename(&tmp, &dst).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return json::internal();
    }
    // Executables must stay executable for the guest (tmp umask may strip).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if rel == "zw" || rel == "engram" {
            let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755));
        }
    }
    let entry = shared::binaries::BinaryEntry {
        path: rel,
        sha256: shared::binaries::sha256_bytes(&bytes),
        size: bytes.len() as u64,
    };
    json::ok(serde_json::json!({ "path": entry.path, "sha256": entry.sha256, "size": entry.size }))
}

async fn binaries_push_one(
    state: &AppState,
    runtime: &vm::runtime::VmRuntime,
    tenant_id: &str,
) -> serde_json::Value {
    let was_live = vm::runtime::fc_socket_path(tenant_id).exists();
    if was_live {
        runtime.kill_vm_process(tenant_id).await;
    }
    match runtime.drop_binaries_versions(tenant_id).await {
        Ok(stamp_dropped) => {
            let ip = if was_live {
                state.activity.touch(tenant_id);
                runtime
                    .ensure_running(tenant_id)
                    .await
                    .ok()
                    .and_then(|a| a.split(':').next().map(str::to_string))
            } else {
                None
            };
            serde_json::json!({
                "tenant_id": tenant_id,
                "stamp_dropped": stamp_dropped,
                "re_waked": ip.is_some(),
                "ip": ip,
            })
        }
        Err(e) => {
            if was_live {
                state.activity.touch(tenant_id);
                let _ = runtime.ensure_running(tenant_id).await;
            }
            serde_json::json!({ "tenant_id": tenant_id, "error": e.to_string() })
        }
    }
}

/// POST /api/admin/binaries/push {tenant_id} -> {stamp_dropped, re_waked}.
/// Admin-triggered binary refresh: drops the guest's versions stamp so the
/// next boot re-demands every binary. Binaries NEVER move on their own.
/// Kills when live, re-wakes when live. Sleeping VMs just lose the stamp.
pub async fn binaries_push(
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
    let tenant_id = match body.get("tenant_id").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return json::bad_request("tenant_id required"),
    };
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    json::ok(binaries_push_one(state, &runtime, &tenant_id).await)
}

/// POST /api/admin/binaries/push-bulk {tenant_ids?[], category_id?, all?}
/// -> {results:[...]}. Sequential (loop mounts contend if parallel).
pub async fn binaries_push_bulk(
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
    let tenant_ids: Vec<String> = if let Some(arr) = body.get("tenant_ids").and_then(|v| v.as_array()) {
        arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
    } else if let Some(cat) = body.get("category_id").and_then(|v| v.as_str()) {
        let conn = match state.gdb.conn() {
            Ok(c) => c,
            Err(_) => return json::internal(),
        };
        match db::global::tenants::list(&conn).await {
            Ok(rows) => rows.into_iter().filter(|t| t.category_id == cat).map(|t| t.id).collect(),
            Err(_) => return json::internal(),
        }
    } else if body.get("all").and_then(|v| v.as_bool()).unwrap_or(false) {
        let conn = match state.gdb.conn() {
            Ok(c) => c,
            Err(_) => return json::internal(),
        };
        match db::global::tenants::list(&conn).await {
            Ok(rows) => rows.into_iter().map(|t| t.id).collect(),
            Err(_) => return json::internal(),
        }
    } else {
        return json::bad_request("provide tenant_ids[] or category_id or all:true");
    };
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    let mut results = Vec::new();
    for tid in tenant_ids {
        results.push(binaries_push_one(state, &runtime, &tid).await);
    }
    json::ok(serde_json::json!({ "results": results }))
}

/// DELETE /api/admin/system-prompt?scope=&ref= -> {deleted: bool}.
/// Removes an override row (falls back to the next precedence level).
pub async fn system_prompt_delete(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    // Scope/ref come from the query string or (portal) the JSON body.
    let (mut scope, mut ref_id) = (query_param(&req, "scope"), query_param(&req, "ref"));
    if scope.is_empty() {
        if let Ok(body) = json::read_json(req).await {
            scope = body
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ref_id = body
                .get("ref")
                .and_then(|v| v.as_str())
                .or_else(|| body.get("ref_id").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
        }
    }
    if !["global", "category", "tenant"].contains(&scope.as_str()) {
        return json::bad_request("scope must be global|category|tenant");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match conn
        .execute(
            "DELETE FROM system_prompts WHERE scope = ?1 AND ref_id = ?2",
            (scope.as_str(), ref_id.as_str()),
        )
        .await
    {
        Ok(n) => json::ok(serde_json::json!({"deleted": n > 0})),
        Err(_) => json::internal(),
    }
}

/// GET /api/admin/system-prompt/effective?tenant_id= -> {tenant_id, content,
/// source}. Shows exactly what the tenant's disk gets (tenant > category >
/// global > host-file).
pub async fn system_prompt_effective(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = query_param(&req, "tenant_id");
    if tenant_id.trim().is_empty() {
        return json::bad_request("missing ?tenant_id=");
    }
    let hp = host_prompt_path(state);
    let (content, source) =
        vm::runtime::resolve_system_prompt(&state.gdb, &hp, &tenant_id).await;
    json::ok(serde_json::json!({
        "tenant_id": tenant_id, "content": content, "source": source,
    }))
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
