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
        let _ = runtime.remove_tenant(&id);
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
    // Per-container usage, best-effort (missing container -> null stats).
    let mut stats = std::collections::HashMap::new();
    if let Some(runtime) = &state.runtime {
        for r in &rows {
            if let Some(s) = runtime.container_stats(&r.tenant_id).await {
                stats.insert(
                    r.tenant_id.clone(),
                    serde_json::json!({
                        "cpu_percent": (s.cpu_percent * 10.0).round() / 10.0,
                        "mem_used_bytes": s.mem_used_bytes,
                        "mem_limit_bytes": s.mem_limit_bytes,
                    }),
                );
            }
        }
    }
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
                "stats": stats.get(&r.tenant_id).cloned().unwrap_or(serde_json::Value::Null),
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
/// STOP: `docker stop` the container (SIGTERM). Container, data folder, IP
/// row all kept — only RAM state goes. Next wake recreates fresh on the
/// same IP. The container is simply off; nothing is wiped.
pub async fn vms_stop(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = json::second_last_seg(&req);
    if let Some(runtime) = &state.runtime {
        runtime.kill_vm_process(&tenant_id).await;
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
    json::ok(serde_json::json!({"ok": true, "status": "asleep", "mode": "stop"}))
}

/// POST /api/admin/vms/{tenant_id}/kill -> {ok, status}.
/// KILL: container removed (`rm`), data folder + IP row KEPT — only the
/// container (and its RAM) goes. Next wake recreates fresh on the same IP
/// (new image generation, prompt + binaries re-demanded). Use after image
/// changes or a wedged guest. This is the button that guarantees the tenant
/// starts from down, never resumed.
pub async fn vms_kill(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = json::second_last_seg(&req);
    if let Some(runtime) = &state.runtime {
        runtime.destroy_container(&tenant_id).await;
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
/// the tenant's persistent data folder (bind-mounted at `/data`).
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

/// POST /api/admin/tenants/{id}/disk/resize {size_gb} -> always 400.
/// Container volumes share host disk: there is nothing to resize. Kept as
/// an explicit error so callers learn it instead of silently succeeding.
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
    // Container volumes share host disk: nothing to resize. Kept as a
    // clear error (not a silent success) so the dashboard Expand button
    // explains itself instead of pretending.
    let _ = (state, tenant_id, size_gb);
    json::bad_request("container volumes share host disk and are not resizable")
}

/// POST /api/admin/workspace/sync {tenant_id} -> {written, re_waked}.
/// Pushes the resolved agent system prompt (SYSTEM.md) into the tenant's
/// data folder (`.bak-date` backup). Plain host write — works whether the
/// container runs or not. Restarts a live container so zw picks the prompt
/// up. Works for SLEEPING tenants too (folder on host, no container needed).
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
    let was_live = runtime.is_container_live(&tenant_id).await;
    match runtime.sync_workspace_seed(&tenant_id, force).await {
        Ok(written) => {
            let ip = if was_live {
                state.activity.touch(&tenant_id);
                runtime.restart_container(&tenant_id).await;
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
        let was_live = runtime.is_container_live(&tid).await;
        match runtime.sync_workspace_seed(&tid, force).await {
            Ok(written) => {
                let ip = if was_live {
                    state.activity.touch(&tid);
                    runtime.restart_container(&tid).await;
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
            &state.tenant_data_root,
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

/// PUT /api/admin/binaries/file?path=<rel>[&stage=1] with raw octet-stream
/// body (200MB cap) -> {path, sha256, size, staged, verified}.
/// `X-Content-Sha256` (client-computed) is verified against the received
/// bytes when present: mismatch = 400, nothing lands.
/// Without stage: lands live instantly (VMs pick up on next demand) and
/// single-file versioned units auto-snapshot a version.
/// With stage=1: lands in `.incoming/<rel>` for later diff+merge (folders).
/// Atomic write (tmp + rename).
pub async fn binaries_upload(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let staff = match auth_ctx::require_admin(state, &req).await {
        Ok(s) => s,
        Err(e) => return e,
    };
    let rel = query_param(&req, "path");
    if !shared::binaries::validate_rel(&rel) {
        return json::bad_request("bad path (a-z 0-9 . _ - /, no .., max 128)");
    }
    let staged = query_param(&req, "stage") == "1";
    let expected = req
        .headers()
        .get("x-content-sha256")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    use http_body_util::BodyExt;
    // req borrowed above for headers/query; re-split not needed — headers
    // were read before consuming the body below.
    let bytes = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => return json::bad_request("unreadable body"),
    };
    const LIMIT: usize = 200 * 1024 * 1024;
    if bytes.len() > LIMIT {
        return json::bad_request("body too large (200MB cap)");
    }
    let sha = shared::binaries::sha256_bytes(&bytes);
    if let Some(exp) = expected {
        if exp != sha {
            return json::bad_request("corrupt upload (hash mismatch: bytes differ from X-Content-Sha256)");
        }
    }
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    // Bundle unit for this path: "zw" | "engram" | "zw-assets".
    let unit = rel.split('/').next().unwrap_or("").to_string();
    let dst = if staged {
        // Staging marker records when the first bytes of this bundle landed.
        let marker = dir.join(".incoming").join(&unit).with_extension("upload_started");
        if !marker.exists() {
            if let Some(parent) = marker.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&marker, shared::now_rfc3339());
        }
        staging_unit_path(&dir, &unit).join(rel.strip_prefix(&format!("{unit}/")).unwrap_or(&rel))
    } else {
        dir.join(&rel)
    };
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
    // Live single-file uploads auto-snapshot a version of their whole unit.
    let mut versioned: Option<i64> = None;
    if !staged {
        if versioned_unit(&unit).is_some() {
            let conn = state.gdb.conn().ok();
            if let Some(conn) = conn {
                let now = shared::now_rfc3339();
                if let Ok(no) = db::global::binary_versions::next_no(&conn, &unit).await {
                    let live = live_unit_path(&dir, &unit);
                    let snap = snapshot_unit_path(&dir, &unit, no);
                    if copy_unit(&live, &snap).is_ok() {
                        let manifest = serde_json::to_string(&unit_manifest(&dir, &unit))
                            .unwrap_or_else(|_| "[]".to_string());
                        let note = format!("upload {rel} {}", &sha[..12.min(sha.len())]);
                        if db::global::binary_versions::insert(
                            &conn, &unit, no, &now, &now, &staff.username, &manifest, &note,
                        )
                        .await
                        .is_ok()
                    {
                            let _ = db::global::binary_versions::active_set(&conn, &unit, no, &now).await;
                            let _ = db::global::binary_versions::prune(&conn, &unit).await;
                            versioned = Some(no);
                        }
                    }
                }
            }
        }
    }
    json::ok(serde_json::json!({
        "path": rel, "sha256": sha, "size": bytes.len() as u64,
        "staged": staged, "verified": true, "version_no": versioned,
    }))
}

async fn binaries_push_one(
    state: &AppState,
    runtime: &vm::runtime::VmRuntime,
    tenant_id: &str,
) -> serde_json::Value {
    // Plain host delete — the container may keep running. Restart when live
    // so the fresh boot re-demands immediately.
    let was_live = runtime.is_container_live(tenant_id).await;
    match runtime.drop_binaries_versions(tenant_id).await {
        Ok(stamp_dropped) => {
            let ip = if was_live {
                state.activity.touch(tenant_id);
                runtime.restart_container(tenant_id).await;
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
/// -> {results:[...]}. Sequential (one tenant at a time).
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
/// GET /api/admin/disks -> {disks:[{name,path,size_bytes}]}: per-tenant
/// data folders under the tenant data root (bind-mounted at `/data`).
/// Replaces the old guest-image picker (kernels are gone with Firecracker).
pub async fn disks_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let dir = state.tenant_data_root.clone();
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|_| json::internal());
    let entries = match entries {
        Ok(e) => e,
        Err(e) => return e,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let size = match &state.runtime {
            Some(r) => r.disk_size_bytes(&name).unwrap_or(0),
            None => 0,
        };
        out.push(serde_json::json!({
            "name": name,
            "path": path.to_string_lossy(),
            "size_bytes": size,
        }));
    }
    out.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    json::ok(serde_json::json!({"disks": out, "dir": dir}))
}

// --------------------------------------------------------------------- hub
/// GET /api/admin/hub/grants?owner=<tenant_id> -> {grants, codes, blocked}.
/// Admin view of who can reach an owner's portfolio (grants + active token
/// share metadata; codes themselves are hashed and unrecoverable).
pub async fn hub_grants_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let owner = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|pair| {
            let mut it = pair.splitn(2, '=');
            (it.next() == Some("owner")).then(|| it.next().unwrap_or("").to_string())
        })
        .unwrap_or_default();
    if owner.trim().is_empty() {
        return json::bad_request("missing ?owner=<tenant_id>");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let rows = match db::global::hub::list_owner(&conn, &owner).await {
        Ok(r) => r,
        Err(_) => return json::internal(),
    };
    let now = shared::now_rfc3339();
    let mut grants = Vec::new();
    let mut codes = Vec::new();
    for r in rows {
        if r.revoked || db::global::hub::is_expired(&r.expires_at, &now) {
            continue;
        }
        if let Some(t) = r.grantee_tenant {
            let username = db::global::tenants::get_by_id(&conn, &t)
                .await
                .ok()
                .flatten()
                .map(|u| u.username);
            grants.push(serde_json::json!({
                "tenant": t, "username": username,
                "created_at": r.created_at, "expires_at": r.expires_at,
            }));
        } else {
            codes.push(serde_json::json!({
                "created_at": r.created_at, "expires_at": r.expires_at,
            }));
        }
    }
    let blocked = db::global::hub::list_blocks(&conn, &owner).await.unwrap_or_default();
    json::ok(serde_json::json!({"owner": owner, "grants": grants, "codes": codes, "blocked": blocked}))
}

/// POST /api/admin/hub/grants {owner_tenant, grantee_tenant, expires_at?}
/// -> 201. Admin-created direct grant. The owner may still revoke it from
/// the hub CLI (`grant_revoke`) or veto via blocklist; admin superiority
/// is re-grant power + full visibility here.
pub async fn hub_grants_create(
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
    let owner = str_field(&body, "owner_tenant").unwrap_or_default();
    let grantee = str_field(&body, "grantee_tenant").unwrap_or_default();
    let expires_at = str_field(&body, "expires_at").filter(|s| !s.trim().is_empty());
    if owner.trim().is_empty() || grantee.trim().is_empty() {
        return json::bad_request("owner_tenant and grantee_tenant are required");
    }
    if owner == grantee {
        return json::bad_request("owner and grantee must differ");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    for id in [&owner, &grantee] {
        match db::global::tenants::get_by_id(&conn, id).await {
            Ok(Some(_)) => {}
            _ => return json::bad_request(format!("unknown tenant {id}")),
        }
    }
    if db::global::hub::is_blocked(&conn, &owner, &grantee).await.unwrap_or(false) {
        return json::bad_request("owner blocked this tenant (unblock first)");
    }
    match db::global::hub::create(&conn, &owner, Some(&grantee), None, expires_at.as_deref()).await {
        Ok(id) => json::created(serde_json::json!({"id": id, "owner": owner, "grantee": grantee})),
        Err(_) => json::internal(),
    }
}

/// DELETE /api/admin/hub/grants {owner_tenant, grantee_tenant} -> {revoked}.
pub async fn hub_grants_delete(
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
    let owner = str_field(&body, "owner_tenant").unwrap_or_default();
    let grantee = str_field(&body, "grantee_tenant").unwrap_or_default();
    if owner.trim().is_empty() || grantee.trim().is_empty() {
        return json::bad_request("owner_tenant and grantee_tenant are required");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    match db::global::hub::revoke_grant(&conn, &owner, &grantee).await {
        Ok(revoked) => json::ok(serde_json::json!({"revoked": revoked})),
        Err(_) => json::internal(),
    }
}

// --------------------------------------------------------------------- binary versions
/// Versioned guest binaries (Binaries tab): 5-deep history per bundle with an
/// active (in-play) pointer. Rollback only moves the pointer — never inserts.

/// Bundle names with history. Single files land live; folders stage first.
fn versioned_unit(name: &str) -> Option<bool> {
    match name {
        "zw" | "engram" => Some(false), // single file
        "zw-assets" => Some(true),      // directory
        _ => None,
    }
}

fn live_unit_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    dir.join(name)
}

fn staging_unit_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    dir.join(".incoming").join(name)
}

fn snapshot_unit_path(dir: &std::path::Path, name: &str, version_no: i64) -> std::path::PathBuf {
    dir.join(".versions").join(name).join(format!("v{version_no}"))
}

fn copy_unit(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            let name = e.file_name();
            let d = dst.join(&name);
            if e.path().is_dir() {
                copy_unit(&e.path(), &d)?;
            } else {
                std::fs::copy(e.path(), d)?;
            }
        }
        Ok(())
    } else {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)?;
        Ok(())
    }
}

/// Manifest entries belonging to one bundle unit.
fn unit_manifest(dir: &std::path::Path, name: &str) -> Vec<serde_json::Value> {
    shared::binaries::manifest_cached(dir)
        .into_iter()
        .filter(|e| e.path == name || e.path.starts_with(&format!("{name}/")))
        .map(|e| serde_json::json!({"path": e.path, "sha256": e.sha256, "size": e.size}))
        .collect()
}

fn entry_map(dir: &std::path::Path, name: &str, staged: bool) -> std::collections::HashMap<String, (String, u64)> {
    let base = if staged {
        staging_unit_path(dir, name)
    } else {
        live_unit_path(dir, name)
    };
    let mut out = std::collections::HashMap::new();
    let files: Vec<(String, std::path::PathBuf)> = if base.is_dir() {
        std::fs::read_dir(&base)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| {
                        e.file_name()
                            .to_str()
                            .map(|n| (n.to_string(), e.path()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else if base.is_file() {
        vec![(name.to_string(), base)]
    } else {
        vec![]
    };
    for (rel, full) in files {
        if let Ok(data) = std::fs::read(&full) {
            out.insert(rel, (shared::binaries::sha256_bytes(&data), data.len() as u64));
        }
    }
    out
}

/// GET /api/admin/binaries/versions?name= -> {versions, active}.
pub async fn binary_versions_list(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let name = query_param(&req, "name");
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let rows = match db::global::binary_versions::list(&conn, &name).await {
        Ok(r) => r,
        Err(_) => return json::internal(),
    };
    let versions: Vec<_> = rows
        .into_iter()
        .map(|r| {
            let manifest: serde_json::Value =
                serde_json::from_str(&r.manifest_json).unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "name": r.name, "version_no": r.version_no,
                "uploaded_at": r.uploaded_at, "merged_at": r.merged_at,
                "created_by": r.created_by, "manifest": manifest, "note": r.note,
            })
        })
        .collect();
    let mut active = serde_json::Map::new();
    for n in ["zw", "engram", "zw-assets"] {
        if let Ok(Some(v)) = db::global::binary_versions::active_get(&conn, n).await {
            active.insert(n.to_string(), serde_json::Value::from(v));
        }
    }
    json::ok(serde_json::json!({"versions": versions, "active": active}))
}

/// POST /api/admin/binaries/diff {name} -> {added, removed, changed, unchanged}.
/// Compares staged (.incoming) against live. Single-file bundles diff the file.
pub async fn binaries_diff(
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
    if versioned_unit(&name).is_none() {
        return json::bad_request("name must be zw, engram or zw-assets");
    }
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let staged = entry_map(&dir, &name, true);
    if staged.is_empty() {
        return json::bad_request(format!("nothing staged for {name} (upload first)"));
    }
    let live = entry_map(&dir, &name, false);
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = Vec::new();
    for (rel, (sha, size)) in &staged {
        match live.get(rel) {
            None => added.push(serde_json::json!({"path": rel, "sha256": sha, "size": size})),
            Some((lsha, _)) if lsha != sha => {
                changed.push(serde_json::json!({"path": rel, "old_sha256": lsha, "new_sha256": sha, "size": size}))
            }
            _ => unchanged.push(rel.clone()),
        }
    }
    for rel in live.keys() {
        if !staged.contains_key(rel) {
            removed.push(rel.clone());
        }
    }
    json::ok(serde_json::json!({
        "name": name, "added": added, "removed": removed,
        "changed": changed, "unchanged": unchanged,
    }))
}

/// POST /api/admin/binaries/merge {name, note?} -> {version_no, uploaded_at,
/// merged_at, pruned}. Snapshots the merged state as a new version, replaces
/// live with staging, sets the active pointer. Whole-folder replace: staged
/// files not in live are added, live files missing from staging are deleted.
pub async fn binaries_merge(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    let staff = match auth_ctx::require_admin(state, &req).await {
        Ok(s) => s,
        Err(e) => return e,
    };
    let body = match json::read_json(req).await {
        Ok(b) => b,
        Err(e) => return e,
    };
    let name = str_field(&body, "name").unwrap_or_default();
    let note = str_field(&body, "note").unwrap_or_default();
    let is_dir = match versioned_unit(&name) {
        Some(d) => d,
        None => return json::bad_request("name must be zw, engram or zw-assets"),
    };
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let staging = staging_unit_path(&dir, &name);
    if !staging.exists() {
        return json::bad_request(format!("nothing staged for {name} (upload first)"));
    }
    // uploaded_at = staging start marker (first bytes), else now.
    let marker = dir.join(".incoming").join(&name).with_extension("upload_started");
    let uploaded_at = std::fs::read_to_string(&marker)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(shared::now_rfc3339);
    let now = shared::now_rfc3339();
    let live = live_unit_path(&dir, &name);
    if is_dir {
        if live.exists() {
            let backup = live.with_extension("merge-bak");
            let _ = std::fs::remove_dir_all(&backup);
            if std::fs::rename(&live, &backup).is_err() {
                return json::internal();
            }
            if std::fs::rename(&staging, &live).is_err() {
                let _ = std::fs::rename(&backup, &live);
                return json::internal();
            }
            let _ = std::fs::remove_dir_all(&backup);
        } else if std::fs::rename(&staging, &live).is_err() {
            return json::internal();
        }
    } else if std::fs::rename(&staging, &live).is_err() {
        return json::internal();
    }
    let _ = std::fs::remove_file(&marker);
    // Snapshot the merged (live) state as the new version.
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let version_no = match db::global::binary_versions::next_no(&conn, &name).await {
        Ok(n) => n,
        Err(_) => return json::internal(),
    };
    let snap = snapshot_unit_path(&dir, &name, version_no);
    if copy_unit(&live, &snap).is_err() {
        return json::internal();
    }
    // Executables must stay executable for the guest.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !is_dir {
            let _ = std::fs::set_permissions(&live, std::fs::Permissions::from_mode(0o755));
            let _ = std::fs::set_permissions(&snap.join(&name), std::fs::Permissions::from_mode(0o755));
        }
    }
    let manifest = serde_json::to_string(&unit_manifest(&dir, &name)).unwrap_or_else(|_| "[]".to_string());
    if db::global::binary_versions::insert(
        &conn, &name, version_no, &uploaded_at, &now, &staff.username, &manifest, &note,
    )
    .await
    .is_err()
    {
        return json::internal();
    }
    if db::global::binary_versions::active_set(&conn, &name, version_no, &now).await.is_err() {
        return json::internal();
    }
    let pruned = db::global::binary_versions::prune(&conn, &name).await.unwrap_or(0);
    // Pruned rows lose their snapshot dirs too.
    if pruned > 0 {
        if let Ok(rows) = db::global::binary_versions::list(&conn, &name).await {
            let keep: std::collections::HashSet<i64> = rows.into_iter().map(|r| r.version_no).collect();
            if let Ok(rd) = std::fs::read_dir(dir.join(".versions").join(&name)) {
                for e in rd.flatten() {
                    let n = e.file_name().to_string_lossy().to_string();
                    if let Some(v) = n.strip_prefix('v').and_then(|x| x.parse::<i64>().ok()) {
                        if !keep.contains(&v) {
                            if e.path().is_dir() {
                                let _ = std::fs::remove_dir_all(e.path());
                            } else {
                                let _ = std::fs::remove_file(e.path());
                            }
                        }
                    }
                }
            }
        }
    }
    json::created(serde_json::json!({
        "name": name, "version_no": version_no,
        "uploaded_at": uploaded_at, "merged_at": now, "pruned": pruned,
    }))
}

/// POST /api/admin/binaries/rollback {name, version_no} -> {activated_at}.
/// Restores the snapshot over live and moves the active pointer. No new row.
pub async fn binaries_rollback(
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
    let version_no = body.get("version_no").and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64))).unwrap_or(0);
    let is_dir = match versioned_unit(&name) {
        Some(d) => d,
        None => return json::bad_request("name must be zw, engram or zw-assets"),
    };
    if version_no < 1 {
        return json::bad_request("version_no required");
    }
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    if db::global::binary_versions::get(&conn, &name, version_no).await.map(|r| r.is_none()).unwrap_or(true) {
        return json::not_found(format!("no version v{version_no} for {name}"));
    }
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let snap = snapshot_unit_path(&dir, &name, version_no);
    if !snap.exists() {
        return json::not_found(format!("snapshot v{version_no} for {name} missing on disk"));
    }
    let live = live_unit_path(&dir, &name);
    let backup = live.with_extension("rollback-bak");
    let _ = if live.is_dir() { std::fs::remove_dir_all(&backup).map(|_| ()) } else { std::fs::remove_file(&backup).map(|_| ()) };
    let moved = if live.exists() { std::fs::rename(&live, &backup).is_ok() } else { true };
    if !moved || copy_unit(&snap, &live).is_err() {
        if moved {
            let _ = std::fs::rename(&backup, &live);
        }
        return json::internal();
    }
    let _ = if backup.is_dir() { std::fs::remove_dir_all(&backup).map(|_| ()) } else { std::fs::remove_file(&backup).map(|_| ()) };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !is_dir {
            let _ = std::fs::set_permissions(&live, std::fs::Permissions::from_mode(0o755));
        }
    }
    let now = shared::now_rfc3339();
    if db::global::binary_versions::active_set(&conn, &name, version_no, &now).await.is_err() {
        return json::internal();
    }
    json::ok(serde_json::json!({"name": name, "version_no": version_no, "activated_at": now}))
}

// --------------------------------------------------------------------- binary scope sync
/// Write one versioned bundle into a tenant's disk folder (host path), so a
/// pilot group can run a new version while the rest stays put. Mirrors the
/// system-prompt sync model: single + bulk (tenant_ids > category_id > all),
/// restart-when-live, per-tenant results. Whole-folder replace for zw-assets.
/// The live binaries dir stays the working set; sync never touches it.
async fn binaries_sync_one(
    state: &AppState,
    runtime: &vm::runtime::VmRuntime,
    tenant_id: &str,
    name: &str,
    version_no: i64,
) -> serde_json::Value {
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return serde_json::json!({"tenant_id": tenant_id, "error": "db unavailable"}),
    };
    let row = match db::global::binary_versions::get(&conn, name, version_no).await {
        Ok(Some(r)) => r,
        _ => return serde_json::json!({"tenant_id": tenant_id, "error": format!("no version v{version_no} for {name}")}),
    };
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let snap = snapshot_unit_path(&dir, name, version_no);
    if !snap.exists() {
        return serde_json::json!({"tenant_id": tenant_id, "error": format!("snapshot v{version_no} for {name} missing on disk")});
    }
    let disk = match runtime.data_disk_path(tenant_id) {
        Ok(p) => p,
        Err(e) => return serde_json::json!({"tenant_id": tenant_id, "error": e.to_string()}),
    };
    let guest = disk.join("supervisor").join("binaries").join(name);
    let was_live = runtime.is_container_live(tenant_id).await;
    // Whole-unit replace (dir: remove + copy; file: copy over).
    let written = if snap.is_dir() {
        let _ = std::fs::remove_dir_all(&guest);
        copy_unit(&snap, &guest).is_ok()
    } else {
        copy_unit(&snap, &guest).is_ok()
    };
    if !written {
        if was_live {
            state.activity.touch(tenant_id);
            let _ = runtime.ensure_running(tenant_id).await;
        }
        return serde_json::json!({"tenant_id": tenant_id, "error": "write to tenant disk failed"});
    }
    // Re-pin the guest versions.json from the synced version's manifest.
    if let Ok(text) = std::fs::read_to_string(guest.parent().unwrap_or(&guest).join("versions.json")) {
        if let Ok(mut pins) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text) {
            if let Ok(manifest) = serde_json::from_str::<Vec<serde_json::Value>>(&row.manifest_json) {
                for e in manifest {
                    if let (Some(p), Some(h)) = (
                        e.get("path").and_then(|v| v.as_str()),
                        e.get("sha256").and_then(|v| v.as_str()),
                    ) {
                        // Manifest paths ("zw", "zw-assets/<f>") match guest
                        // pin keys exactly.
                        pins.insert(p.to_string(), serde_json::Value::String(h.to_string()));
                    }
                }
                let _ = std::fs::write(
                    guest.parent().unwrap_or(&guest).join("versions.json"),
                    serde_json::to_string(&pins).unwrap_or_default(),
                );
            }
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if !snap.is_dir() {
            let _ = std::fs::set_permissions(&guest, std::fs::Permissions::from_mode(0o755));
        }
    }
    let ip = if was_live {
        state.activity.touch(tenant_id);
        runtime.restart_container(tenant_id).await;
        runtime
            .ensure_running(tenant_id)
            .await
            .ok()
            .and_then(|a| a.split(':').next().map(str::to_string))
    } else {
        None
    };
    serde_json::json!({
        "tenant_id": tenant_id, "name": name, "version_no": version_no,
        "written": true, "re_waked": ip.is_some(), "ip": ip,
    })
}

/// POST /api/admin/binaries/sync {tenant_id, name, version_no} -> single result.
pub async fn binaries_sync(
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
    let tenant_id = body.get("tenant_id").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let version_no = body.get("version_no").and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64))).unwrap_or(0);
    if tenant_id.is_empty() || versioned_unit(&name).is_none() || version_no < 1 {
        return json::bad_request("tenant_id, name (zw|engram|zw-assets) and version_no required");
    }
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    json::ok(binaries_sync_one(state, &runtime, &tenant_id, &name, version_no).await)
}

/// POST /api/admin/binaries/sync-bulk {name, version_no, tenant_ids?[], category_id?, all?}
/// -> {results:[...]}. Sequential, like workspace sync-bulk.
pub async fn binaries_sync_bulk(
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
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let version_no = body.get("version_no").and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64))).unwrap_or(0);
    if versioned_unit(&name).is_none() || version_no < 1 {
        return json::bad_request("name (zw|engram|zw-assets) and version_no required");
    }
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
        return json::ok(serde_json::json!({"results": [], "name": name, "version_no": version_no}));
    }
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    let mut results = Vec::new();
    for tid in tenant_ids {
        results.push(binaries_sync_one(state, &runtime, &tid, &name, version_no).await);
    }
    json::ok(serde_json::json!({"results": results, "name": name, "version_no": version_no}))
}

/// GET /api/admin/binaries/effective?tenant_id= -> {bundles:{name:{version_no?,state}}}.
/// Resolves what a tenant actually runs by matching guest pins against known
/// version manifests (newest first), else the live working set, else custom.
pub async fn binaries_effective(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> ApiResponse {
    if let Err(e) = auth_ctx::require_admin(state, &req).await {
        return e;
    }
    let tenant_id = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|pair| {
            let mut it = pair.splitn(2, '=');
            (it.next() == Some("tenant_id")).then(|| it.next().unwrap_or("").to_string())
        })
        .unwrap_or_default();
    if tenant_id.trim().is_empty() {
        return json::bad_request("missing ?tenant_id=");
    }
    let runtime = match &state.runtime {
        Some(r) => r.clone(),
        None => return json::unavailable("vm runtime not configured"),
    };
    let disk = match runtime.data_disk_path(&tenant_id) {
        Ok(p) => p,
        Err(e) => return json::bad_request(e.to_string()),
    };
    let pins: serde_json::Map<String, serde_json::Value> = std::fs::read_to_string(
        disk.join("supervisor").join("binaries").join("versions.json"),
    )
    .ok()
    .and_then(|t| serde_json::from_str(&t).ok())
    .unwrap_or_default();
    let conn = match state.gdb.conn() {
        Ok(c) => c,
        Err(_) => return json::internal(),
    };
    let dir = std::path::PathBuf::from(&state.binaries_dir);
    let mut bundles = serde_json::Map::new();
    for name in ["zw", "engram", "zw-assets"] {
        // Guest pins for this bundle (manifest keys are unit-relative).
        let guest: std::collections::HashMap<String, String> = pins
            .iter()
            .filter(|(k, _)| *k == name || k.starts_with(&format!("{name}/")))
            .filter_map(|(k, v)| v.as_str().map(|h| (k.clone(), h.to_string())))
            .collect();
        let mut resolved = serde_json::json!({"state": "unknown"});
        if !guest.is_empty() {
            if let Ok(rows) = db::global::binary_versions::list(&conn, name).await {
                for r in rows {
                    if let Ok(manifest) = serde_json::from_str::<Vec<serde_json::Value>>(&r.manifest_json) {
                        let want: Vec<(String, String)> = manifest
                            .iter()
                            .filter_map(|e| {
                                Some((
                                    e.get("path")?.as_str()?.to_string(),
                                    e.get("sha256")?.as_str()?.to_string(),
                                ))
                            })
                            .collect();
                        if !want.is_empty()
                            && want.iter().all(|(p, h)| guest.get(p).is_some_and(|g| g == h))
                        {
                            resolved = serde_json::json!({"version_no": r.version_no, "state": "versioned"});
                            break;
                        }
                    }
                }
            }
            if resolved.get("state").and_then(|v| v.as_str()) == Some("unknown") {
                let live: std::collections::HashMap<String, String> = unit_manifest(&dir, name)
                    .into_iter()
                    .filter_map(|e| {
                        Some((
                            e.get("path")?.as_str()?.to_string(),
                            e.get("sha256")?.as_str()?.to_string(),
                        ))
                    })
                    .collect();
                if !live.is_empty() && live.iter().all(|(p, h)| guest.get(p).is_some_and(|g| g == h)) {
                    resolved = serde_json::json!({"state": "live"});
                } else {
                    resolved = serde_json::json!({"state": "custom"});
                }
            }
        }
        bundles.insert(name.to_string(), resolved);
    }
    json::ok(serde_json::json!({"tenant_id": tenant_id, "bundles": bundles}))
}
