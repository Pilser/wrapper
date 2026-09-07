//! Link an OAuth identity to a tenant account.

use telemetry::{Level, Telemetry};

/// Record an OAuth identity link for a tenant.
///
/// Still a stub: the link is only validated and audited, not persisted yet.
/// Empty `tenant_id`/`provider`/`subject` values are rejected with
/// `InvalidInput`, and both outcomes emit audit telemetry following the
/// `permissions/enforce.rs` pattern.
pub fn link_account(
    telemetry: &Telemetry,
    tenant_id: &str,
    provider: &str,
    subject: &str,
) -> anyhow::Result<()> {
    for (name, value) in [
        ("tenant_id", tenant_id),
        ("provider", provider),
        ("subject", subject),
    ] {
        if value.trim().is_empty() {
            telemetry
                .builder()
                .level(Level::Warn)
                .kind("auth.oauth.link_failed")
                .category("auth")
                .message(format!("oauth link with empty {name}"))
                .emit();
            return Err(
                core::Error::InvalidInput(format!("oauth link {name} must not be empty")).into(),
            );
        }
    }
    telemetry
        .builder()
        .level(Level::Info)
        .kind("auth.oauth.linked")
        .category("auth")
        .tenant_id(Some(tenant_id.to_string()))
        .message(format!("oauth identity linked for tenant {tenant_id} via {provider}"))
        .emit();
    Ok(())
}
