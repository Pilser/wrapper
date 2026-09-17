//! CRUD for `hub_shares` + `hub_blocks` (hubportfolio cross-tenant access).

use serde::{Deserialize, Serialize};
use turso::Connection;

/// One hub share row: direct grant (`grantee_tenant` set) or token share
/// (`code_hash` set). `code_hash` is sha256 hex — the plaintext code is
/// shown once at creation and never stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubShare {
    pub id: String,
    pub owner_tenant: String,
    pub grantee_tenant: Option<String>,
    pub code_hash: Option<String>,
    pub scope: String,
    pub expires_at: Option<String>,
    pub revoked: bool,
    pub created_at: String,
}

fn row(r: &turso::Row) -> HubShare {
    let text = |i: usize| {
        r.get_value(i)
            .ok()
            .and_then(|v| v.as_text().cloned())
            .unwrap_or_default()
    };
    let opt = |i: usize| {
        r.get_value(i)
            .ok()
            .and_then(|v| v.as_text().cloned())
            .filter(|s| !s.is_empty())
    };
    HubShare {
        id: text(0),
        owner_tenant: text(1),
        grantee_tenant: opt(2),
        code_hash: opt(3),
        scope: { let s = text(4); if s.is_empty() { "edit".to_string() } else { s } },
        expires_at: opt(5),
        revoked: r.get_value(6).ok().and_then(|v| v.as_integer().copied()).unwrap_or(0) != 0,
        created_at: text(7),
    }
}

const COLS: &str = "id, owner_tenant, grantee_tenant, code_hash, scope, expires_at, revoked, created_at";

/// True when `expires_at` is set and already past (RFC3339 compares lexically).
pub fn is_expired(expires_at: &Option<String>, now: &str) -> bool {
    expires_at.as_deref().is_some_and(|e| e < now)
}

pub async fn create(
    conn: &Connection,
    owner: &str,
    grantee: Option<&str>,
    code_hash: Option<&str>,
    expires_at: Option<&str>,
) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO hub_shares (id, owner_tenant, grantee_tenant, code_hash, scope, expires_at, revoked, created_at) \
         VALUES (?1,?2,?3,?4,'edit',?5,0,?6)",
        (
            id.as_str(),
            owner,
            grantee.unwrap_or(""),
            code_hash.unwrap_or(""),
            expires_at.unwrap_or(""),
            crate::now_str().as_str(),
        ),
    )
    .await?;
    Ok(id)
}

/// Every non-revoked row for an owner (grants + token shares, newest first).
pub async fn list_owner(conn: &Connection, owner: &str) -> anyhow::Result<Vec<HubShare>> {
    let mut rows = conn
        .query(&format!("SELECT {COLS} FROM hub_shares WHERE owner_tenant = ?1 ORDER BY created_at DESC"), [owner])
        .await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        out.push(row(&r));
    }
    Ok(out)
}

/// Direct grant owner -> grantee (active only).
pub async fn find_grant(conn: &Connection, owner: &str, grantee: &str) -> anyhow::Result<Option<HubShare>> {
    let mut rows = conn
        .query(
            &format!("SELECT {COLS} FROM hub_shares WHERE owner_tenant = ?1 AND grantee_tenant = ?2 AND revoked = 0"),
            (owner, grantee),
        )
        .await?;
    let mut out = rows.next().await?.map(|r| row(&r));
    if let Some(s) = &out {
        if is_expired(&s.expires_at, &crate::now_str()) {
            out = None;
        }
    }
    Ok(out)
}

/// Token share for an owner by code hash (active only, grantee IS NULL).
pub async fn find_token(conn: &Connection, owner: &str, code_hash: &str) -> anyhow::Result<Option<HubShare>> {
    let mut rows = conn
        .query(
            &format!("SELECT {COLS} FROM hub_shares WHERE owner_tenant = ?1 AND code_hash = ?2 AND revoked = 0"),
            (owner, code_hash),
        )
        .await?;
    let mut out = rows.next().await?.map(|r| row(&r));
    if let Some(s) = &out {
        if s.grantee_tenant.is_some() || is_expired(&s.expires_at, &crate::now_str()) {
            out = None;
        }
    }
    Ok(out)
}

pub async fn revoke_code(conn: &Connection, owner: &str, code_hash: &str) -> anyhow::Result<bool> {
    let n = conn
        .execute(
            "UPDATE hub_shares SET revoked = 1 WHERE owner_tenant = ?1 AND code_hash = ?2 AND revoked = 0",
            (owner, code_hash),
        )
        .await?;
    Ok(n > 0)
}

pub async fn revoke_grant(conn: &Connection, owner: &str, grantee: &str) -> anyhow::Result<bool> {
    let n = conn
        .execute(
            "UPDATE hub_shares SET revoked = 1 WHERE owner_tenant = ?1 AND grantee_tenant = ?2 AND revoked = 0",
            (owner, grantee),
        )
        .await?;
    Ok(n > 0)
}

pub async fn block(conn: &Connection, owner: &str, blocked: &str) -> anyhow::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO hub_blocks (owner_tenant, blocked_tenant, created_at) VALUES (?1,?2,?3)",
        (owner, blocked, crate::now_str().as_str()),
    )
    .await?;
    Ok(())
}

pub async fn unblock(conn: &Connection, owner: &str, blocked: &str) -> anyhow::Result<bool> {
    let n = conn
        .execute(
            "DELETE FROM hub_blocks WHERE owner_tenant = ?1 AND blocked_tenant = ?2",
            (owner, blocked),
        )
        .await?;
    Ok(n > 0)
}

pub async fn is_blocked(conn: &Connection, owner: &str, tenant: &str) -> anyhow::Result<bool> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM hub_blocks WHERE owner_tenant = ?1 AND blocked_tenant = ?2",
            (owner, tenant),
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

pub async fn list_blocks(conn: &Connection, owner: &str) -> anyhow::Result<Vec<String>> {
    let mut rows = conn
        .query("SELECT blocked_tenant FROM hub_blocks WHERE owner_tenant = ?1", [owner])
        .await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        if let Some(v) = r.get_value(0).ok().and_then(|v| v.as_text().cloned()) {
            out.push(v);
        }
    }
    Ok(out)
}
