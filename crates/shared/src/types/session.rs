//! Authentication sessions and tokens.

use serde::{Deserialize, Serialize};

/// A browser/session held by a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub tenant_id: String,
    pub token: String,
    pub ip_address: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub last_active: String,
}

/// An individually minted auth token (refresh/bearer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub tenant_id: String,
    pub expires_at: String,
    #[serde(default)]
    pub revoked: bool,
}