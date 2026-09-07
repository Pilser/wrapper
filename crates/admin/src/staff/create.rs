//! Create a staff account (row in the global `staff` table).

use shared::{now_rfc3339, Error, Result};
use telemetry::{Level, Telemetry};

/// A staff account with dashboard / oversight duties.
#[derive(Debug, Clone)]
pub struct StaffMember {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub role: String,
    pub created_at: String,
}

/// Insert the staff row via `db::global::staff::create` and emit
/// `admin.staff.created`.
pub async fn create_staff(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    username: &str,
    password_hash: &str,
    display_name: Option<String>,
    role: &str,
) -> Result<StaffMember> {
    if username.trim().is_empty() {
        return Err(Error::InvalidInput(
            "username must not be empty".into(),
        ));
    }
    if password_hash.trim().is_empty() {
        return Err(Error::InvalidInput(
            "password_hash must not be empty".into(),
        ));
    }
    if role.trim().is_empty() {
        return Err(Error::InvalidInput("role must not be empty".into()));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    if db::global::staff::get_by_username(&conn, username)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .is_some()
    {
        return Err(Error::AlreadyExists(format!("staff {username}")));
    }
    let now = now_rfc3339();
    let member = StaffMember {
        id: format!("staff_{}", uuid::Uuid::new_v4()),
        username: username.to_string(),
        display_name,
        role: role.to_string(),
        created_at: now.clone(),
    };
    let row = db::global::StaffRow {
        id: member.id.clone(),
        username: member.username.clone(),
        password_hash: password_hash.to_string(),
        role: member.role.clone(),
        created_at: now,
    };
    db::global::staff::create(&conn, &row)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.staff.created")
        .category("admin")
        .source("admin::staff::create")
        .message(format!("staff {} created", member.id))
        .data("staff_id", member.id.clone())
        .tag("admin")
        .tag("staff")
        .emit();
    Ok(member)
}
