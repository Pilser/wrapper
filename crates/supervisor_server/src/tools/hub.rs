//! `hub.*` ops: cross-tenant portfolio access for the `hubportfolio` CLI.
//!
//! Transport: same TCP NDJSON channel as `portfolio.*`, but routed here
//! (never through `dispatch_tool`, never in MCP `TOOL_NAMES`): hub is
//! CLI-only by design, invisible to agents.
//!
//! Caller identity = TCP peer IP mapped through the `networks` table
//! (spoof-proof: the bridge assigns the IP, not the guest). The envelope
//! tenant must match the mapped tenant; unmapped peers (local dev) fall
//! back to the envelope claim.
//!
//! Access order per request: owner == caller > owner blocklist (deny,
//! overrides everything incl. admin grants) > direct grant
//! (owner->grantee row, made by owner CLI or admin portal) > token share
//! (owner-made short code). Token codes are stored sha256-hashed.

use serde_json::Value;
use sha2::{Digest, Sha256};
use shared::{Error, Result};
use telemetry::{Level, Telemetry};

use crate::protocol::message::Envelope;

/// Port the in-guest engram API listens on (implemented guest-side).
const ENGRAM_PORT: u16 = 4545;
/// Alphabet without lookalikes (no 0/o/1/l/i).
const CODE_ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

fn sha_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

fn gen_code() -> String {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    bytes
        .iter()
        .take(4)
        .map(|b| CODE_ALPHABET[(b % CODE_ALPHABET.len() as u8) as usize] as char)
        .collect()
}

fn peer_ip(peer: &str) -> &str {
    match peer.rsplit_once(':') {
        Some((ip, _)) if ip.contains('.') || ip.starts_with('[') => {
            ip.trim_start_matches('[').trim_end_matches(']')
        }
        _ => peer,
    }
}

/// Resolve the calling tenant: peer IP mapping wins, envelope claim is the
/// fallback. A mapped IP claiming a different tenant is rejected.
async fn caller_tenant(db: &db::Db, peer: &str, claimed: &str) -> Result<String> {
    let gconn = db.global().conn().map_err(|e| Error::Database(e.to_string()))?;
    let mapped = db::global::networks::all(&gconn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .into_iter()
        .find(|r| r.ip_address == peer_ip(peer))
        .map(|r| r.tenant_id);
    match (mapped, claimed.trim()) {
        (Some(m), c) if !c.is_empty() && c != m => Err(Error::Unauthorized(format!(
            "peer {m} may not claim tenant {c}"
        ))),
        (Some(m), _) => Ok(m),
        (None, c) if !c.is_empty() => Ok(c.to_string()),
        (None, _) => Err(Error::InvalidInput("cannot identify calling tenant".to_string())),
    }
}

/// Enforce hub access of `caller` to `target`'s portfolio.
async fn check_access(
    gconn: &turso::Connection,
    caller: &str,
    target: &str,
    code: Option<&str>,
) -> Result<()> {
    if caller == target {
        return Ok(());
    }
    if db::global::hub::is_blocked(gconn, target, caller)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
    {
        return Err(Error::Unauthorized(format!("tenant {target} blocked you")));
    }
    if db::global::hub::find_grant(gconn, target, caller)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .is_some()
    {
        return Ok(());
    }
    if let Some(c) = code {
        if db::global::hub::find_token(gconn, target, &sha_hex(c))
            .await
            .map_err(|e| Error::Database(e.to_string()))?
            .is_some()
        {
            return Ok(());
        }
        return Err(Error::Unauthorized(format!(
            "invalid share code for tenant {target}"
        )));
    }
    Err(Error::Unauthorized(format!(
        "no hub access to tenant {target} (need an admin/owner grant or share code)"
    )))
}

async fn username_of(gconn: &turso::Connection, tenant_id: &str) -> Option<String> {
    let t = db::global::tenants::get_by_id(gconn, tenant_id).await.ok()??;
    let name = t.display_name.filter(|s| !s.is_empty());
    Some(match name {
        Some(n) => format!("{} ({n})", t.username),
        None => t.username,
    })
}

async fn target_ip(gconn: &turso::Connection, target: &str) -> Result<String> {
    db::global::networks::all(gconn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .into_iter()
        .find(|r| r.tenant_id == target)
        .map(|r| r.ip_address)
        .ok_or_else(|| Error::NotFound(format!("no address for tenant {target}")))
}

/// Timeline note on the owner's portfolio (grant/revoke/share visibility).
async fn note(db: &db::Db, owner: &str, summary: &str) {
    if let Ok(tdb) = db.tenant(owner).await {
        if let Ok(conn) = tdb.conn() {
            let now = shared::now_rfc3339();
            db::tenant::timeline::insert(&conn, "hub", None, summary, Some("hub"), &now)
                .await
                .ok();
        }
    }
}

fn emit(telemetry: &Telemetry, kind: &str, msg: String) {
    telemetry
        .builder()
        .level(Level::Info)
        .kind(kind)
        .category("hub")
        .message(msg)
        .emit();
}

/// Route one `hub.*` envelope. `peer` is the TCP peer (`ip:port`).
pub async fn handle_hub_op(
    envelope: &Envelope,
    peer: &str,
    db: &db::Db,
    telemetry: &Telemetry,
    runtime: &Option<vm::runtime::VmRuntime>,
) -> Result<Value> {
    let full_op = envelope
        .payload
        .get("op")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let verb = full_op.strip_prefix("hub.").ok_or_else(|| {
        Error::InvalidInput(format!("not a hub op {full_op:?}"))
    })?;
    // Hoist data.* flat (same as portfolio path) so handlers read args alike.
    let mut payload = envelope.payload.clone();
    if let Some(obj) = payload.as_object_mut() {
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()).cloned() {
            for (k, v) in data {
                obj.entry(k).or_insert(v);
            }
        }
    }
    let arg = |k: &str| super::str_arg(&payload, k).map(|s| s.to_string());
    let caller = caller_tenant(db, peer, &envelope.tenant_id).await?;
    let gconn = db.global().conn().map_err(|e| Error::Database(e.to_string()))?;

    match verb {
        // ---- owner-only: share codes ----
        "share_create" => {
            let expires_at = arg("expires_at").filter(|s| !s.trim().is_empty());
            let (code, hash) = loop {
                let c = gen_code();
                let h = sha_hex(&c);
                if db::global::hub::find_token(&gconn, &caller, &h)
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?
                    .is_none()
                {
                    break (c, h);
                }
            };
            db::global::hub::create(&gconn, &caller, None, Some(&hash), expires_at.as_deref())
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            note(db, &caller, "hub share code created").await;
            emit(telemetry, "hub.share.created", format!("{caller} created a hub share code"));
            Ok(serde_json::json!({"code": code, "expires_at": expires_at}))
        }
        "share_list" => {
            let rows = db::global::hub::list_owner(&gconn, &caller)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            let mut codes = Vec::new();
            let mut grants = Vec::new();
            for r in rows {
                if r.revoked {
                    continue;
                }
                if let Some(t) = r.grantee_tenant {
                    grants.push(serde_json::json!({
                        "tenant": t,
                        "username": username_of(&gconn, &t).await,
                        "created_at": r.created_at, "expires_at": r.expires_at,
                    }));
                } else {
                    codes.push(serde_json::json!({
                        "created_at": r.created_at, "expires_at": r.expires_at,
                    }));
                }
            }
            Ok(serde_json::json!({"owner": caller, "grants": grants, "codes": codes}))
        }
        "share_cancel" => {
            let code = arg("code").ok_or_else(|| Error::InvalidInput("hub.share_cancel missing code".to_string()))?;
            let ok = db::global::hub::revoke_code(&gconn, &caller, &sha_hex(&code))
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            note(db, &caller, "hub share code cancelled").await;
            Ok(serde_json::json!({"cancelled": ok}))
        }
        // ---- owner-only: who can reach me / blocks ----
        "access" => {
            let rows = db::global::hub::list_owner(&gconn, &caller)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            let mut grants = Vec::new();
            let mut active_codes = 0;
            for r in rows {
                if r.revoked || db::global::hub::is_expired(&r.expires_at, &shared::now_rfc3339()) {
                    continue;
                }
                if let Some(t) = r.grantee_tenant {
                    grants.push(serde_json::json!({
                        "tenant": t,
                        "username": username_of(&gconn, &t).await,
                        "created_at": r.created_at, "expires_at": r.expires_at,
                    }));
                } else {
                    active_codes += 1;
                }
            }
            let blocked_ids = db::global::hub::list_blocks(&gconn, &caller)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            let mut blocked = Vec::new();
            for t in blocked_ids {
                blocked.push(serde_json::json!({
                    "tenant": t, "username": username_of(&gconn, &t).await,
                }));
            }
            Ok(serde_json::json!({
                "owner": caller, "grants": grants,
                "active_codes": active_codes, "blocked": blocked,
            }))
        }
        "block" => {
            let t = arg("tenant").ok_or_else(|| Error::InvalidInput("hub.block missing tenant".to_string()))?;
            if t == caller {
                return Err(Error::InvalidInput("cannot block yourself".to_string()));
            }
            db::global::hub::block(&gconn, &caller, &t)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            note(db, &caller, &format!("hub blocked tenant {t}")).await;
            Ok(serde_json::json!({"blocked": t}))
        }
        "unblock" => {
            let t = arg("tenant").ok_or_else(|| Error::InvalidInput("hub.unblock missing tenant".to_string()))?;
            let ok = db::global::hub::unblock(&gconn, &caller, &t)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            Ok(serde_json::json!({"unblocked": ok}))
        }
        "grant_revoke" => {
            let t = arg("tenant").ok_or_else(|| Error::InvalidInput("hub.grant_revoke missing tenant".to_string()))?;
            let ok = db::global::hub::revoke_grant(&gconn, &caller, &t)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            note(db, &caller, &format!("hub access revoked for tenant {t}")).await;
            Ok(serde_json::json!({"revoked": ok}))
        }
        // ---- wake the target container ----
        "activate" => {
            let target = arg("target_tenant").ok_or_else(|| Error::InvalidInput("hub.activate missing target_tenant".to_string()))?;
            let code = arg("code").filter(|s| !s.is_empty());
            check_access(&gconn, &caller, &target, code.as_deref()).await?;
            let rt = runtime.as_ref().ok_or_else(|| {
                Error::InvalidInput("container runtime unavailable (degraded mode)".to_string())
            })?;
            let addr = rt.ensure_running(&target).await.map_err(Error::from)?;
            emit(telemetry, "hub.activate", format!("{caller} activated {target} -> {addr}"));
            Ok(serde_json::json!({"target": target, "status": "active", "endpoint": addr}))
        }
        // ---- live engram API proxy (target must be active) ----
        "engram" => {
            let target = arg("target_tenant").ok_or_else(|| Error::InvalidInput("hub.engram missing target_tenant".to_string()))?;
            let code = arg("code").filter(|s| !s.is_empty());
            check_access(&gconn, &caller, &target, code.as_deref()).await?;
            let path = arg("path").unwrap_or_else(|| "health".to_string());
            let path = path.trim_start_matches('/').to_string();
            if path.is_empty() || path.starts_with('.') || path.contains("..") {
                return Err(Error::InvalidInput("bad engram path".to_string()));
            }
            // A JSON null body counts as absent (typed CLIs send explicit nulls).
            let has_body = payload.get("body").is_some_and(|b| !b.is_null());
            let method = arg("method").unwrap_or_else(|| {
                if has_body { "POST".to_string() } else { "GET".to_string() }
            }).to_uppercase();
            if method != "GET" && method != "POST" {
                return Err(Error::InvalidInput("engram method must be GET or POST".to_string()));
            }
            let ip = target_ip(&gconn, &target).await.map_err(|_| {
                Error::InvalidInput(format!(
                    "target {target} has no address yet — run `hubportfolio activate {target}` first"
                ))
            })?;
            let url = format!("http://{ip}:{ENGRAM_PORT}/{path}");
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| Error::Internal(e.to_string()))?;
            let req = if method == "POST" {
                let mut r = client.post(&url);
                if let Some(body) = payload.get("body").filter(|b| !b.is_null()) {
                    r = r.json(body);
                }
                r
            } else {
                client.get(&url)
            };
            let resp = req.send().await.map_err(|e| {
                Error::InvalidInput(format!(
                    "target engram not reachable at {ip}:{ENGRAM_PORT} ({e}) — run `hubportfolio activate {target}` first"
                ))
            })?;
            let status = resp.status().as_u16();
            let bytes = resp.bytes().await.map_err(|e| Error::Internal(e.to_string()))?;
            let body: Value = serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()));
            emit(telemetry, "hub.engram", format!("{caller} -> {target} engram {method} /{path} -> {status}"));
            Ok(serde_json::json!({"target": target, "status": status, "body": body}))
        }
        // ---- full portfolio mirror: hub.<verb> behaves like portfolio <verb> on the target ----
        _ => {
            let target = arg("target_tenant").ok_or_else(|| Error::InvalidInput(format!("{full_op} missing target_tenant (run `hubportfolio use <id> [code]` first)")))?;
            let code = arg("code").filter(|s| !s.is_empty());
            check_access(&gconn, &caller, &target, code.as_deref()).await?;
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("op".to_string(), Value::String(verb.to_string()));
            }
            emit(telemetry, "hub.portfolio", format!("{caller} -> {target} {verb}"));
            super::portfolio::handle_portfolio_call(&target, &payload, db, telemetry).await
        }
    }
}
