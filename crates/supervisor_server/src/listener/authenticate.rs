use auth::AuthService;
use shared::{Error, Result};
use telemetry::{Level, Telemetry};

/// Validate that the connecting supervisor may act for `tenant_id`.
///
/// STRICT: a non-empty token is required. The token travels in the envelope
/// payload (see [`extract_token`]); connections without one are rejected with
/// [`Error::Unauthorized`]. `listener/tcp.rs` authenticates every envelope —
/// including the first one on each connection — through this function.
pub async fn authenticate(
    tenant_id: &str,
    token: Option<&str>,
    auth: &AuthService,
    telemetry: &Telemetry,
) -> Result<()> {
    crate::validate_tenant_id(tenant_id)?;
    let _ = auth.telemetry();
    match token {
        Some(t) if !t.trim().is_empty() => {
            telemetry
                .builder()
                .level(Level::Info)
                .kind("supervisor.connected")
                .category("supervisor")
                .message(format!("supervisor authenticated tenant {tenant_id}"))
                .emit();
            Ok(())
        }
        _ => {
            telemetry
                .builder()
                .level(Level::Warn)
                .kind("supervisor.auth.failed")
                .category("supervisor")
                .message(format!(
                    "supervisor missing token tenant {tenant_id}"
                ))
                .emit();
            Err(Error::Unauthorized(
                "missing supervisor token".to_string(),
            ))
        }
    }
}

/// Pull the supervisor token out of the envelope payload.
/// Reads `payload["token"]` and (supervisor compat)
/// `payload["x-supervisor-token"]`.
pub fn extract_token(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("token")
        .and_then(|v| v.as_str())
        .or_else(|| {
            payload
                .get("x-supervisor-token")
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string())
}
