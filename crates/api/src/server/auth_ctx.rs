//! Session resolution for `/api` handlers.
//!
//! Two principals share the `sessions` table, distinguished by id prefix:
//! * tenants: `sessions.tenant_id` = tenant id (from `/api/auth/*`)
//! * staff:   `sessions.tenant_id` = `staff:<staff-id>` (from admin login)
//!
//! Staff rows carry `role`; only `admin` may call `/api/admin/*` (except the
//! one-time bootstrap + staff login).

use chrono::{DateTime, Utc};
use shared::Tenant;

use super::json;
use super::{ApiResponse, AppState};

/// Who is calling.
pub enum AuthContext {
    Tenant(Tenant),
    Staff(db::global::StaffRow),
}

impl AuthContext {
    pub fn is_admin(&self) -> bool {
        matches!(self, AuthContext::Staff(s) if s.role == "admin")
    }
}

/// Resolve Bearer -> principal. None = 401 (also deletes expired sessions).
pub async fn resolve(
    state: &AppState,
    req: &hyper::Request<hyper::body::Incoming>,
) -> Result<AuthContext, ApiResponse> {
    let token = json::bearer(req).ok_or_else(json::unauthorized)?;
    let conn = state.gdb.conn().map_err(|_| json::internal())?;
    let session = db::global::sessions::get_by_token(&conn, &token)
        .await
        .map_err(|_| json::internal())?;
    let session = session.ok_or_else(json::unauthorized)?;
    if session_expired(&session.expires_at) {
        let _ = db::global::sessions::delete(&conn, &token).await;
        return Err(json::unauthorized());
    }
    if let Some(staff_id) = session.tenant_id.strip_prefix("staff:") {
        let staff = db::global::staff::get_by_id(&conn, staff_id)
            .await
            .map_err(|_| json::internal())?
            .ok_or_else(json::unauthorized)?;
        return Ok(AuthContext::Staff(staff));
    }
    let tenant = db::global::tenants::get_by_id(&conn, &session.tenant_id)
        .await
        .map_err(|_| json::internal())?
        .ok_or_else(json::unauthorized)?;
    Ok(AuthContext::Tenant(tenant))
}

/// Require an admin principal.
pub async fn require_admin(
    state: &AppState,
    req: &hyper::Request<hyper::body::Incoming>,
) -> Result<db::global::StaffRow, ApiResponse> {
    match resolve(state, req).await? {
        AuthContext::Staff(s) if s.role == "admin" => Ok(s),
        AuthContext::Staff(_) => Err(json::forbidden("admin role required")),
        AuthContext::Tenant(_) => Err(json::forbidden("admin role required")),
    }
}

fn session_expired(expires_at: &str) -> bool {
    expires_at
        .parse::<DateTime<Utc>>()
        .map(|exp| exp <= Utc::now())
        .unwrap_or(true)
}

/// Mint + persist a session (24h). `owner` is a tenant id or `staff:<id>`.
pub async fn mint_session(
    state: &AppState,
    owner: &str,
) -> Result<String, ApiResponse> {
    let token = format!("tk_{}", uuid::Uuid::new_v4().simple());
    let now = shared::now_rfc3339();
    let expires = (Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    let session = shared::Session {
        id: format!("sess_{}", &token[3..11]),
        tenant_id: owner.to_string(),
        token: token.clone(),
        ip_address: None,
        created_at: now.clone(),
        expires_at: expires,
        last_active: now,
    };
    let conn = state.gdb.conn().map_err(|_| json::internal())?;
    db::global::sessions::insert(&conn, &session)
        .await
        .map_err(|_| json::internal())?;
    Ok(token)
}

/// Public tenant view for frontends (never password_hash).
pub fn tenant_public(t: &Tenant) -> serde_json::Value {
    serde_json::json!({
        "id": t.id,
        "username": t.username,
        "display_name": t.display_name,
        "category_id": t.category_id,
        "status": t.status.as_str(),
        "vm_ip": t.vm_ip,
        "created_at": t.created_at,
        "last_login": t.last_login,
    })
}
