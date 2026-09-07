//! OAuth callback handling.

use super::provider::OAuthProvider;
use telemetry::{Level, Telemetry};

/// Validate an OAuth authorization callback and return the linked subject.
///
/// Still a stub: no token exchange is performed yet, so a successful callback
/// yields an empty subject. All inputs are validated (`InvalidInput` on
/// empty `code`/`state`/provider ids) and both outcomes emit audit telemetry
/// following the `permissions/enforce.rs` pattern.
pub fn handle_callback(
    telemetry: &Telemetry,
    provider: &OAuthProvider,
    code: &str,
    state: &str,
) -> anyhow::Result<String> {
    if provider.name.trim().is_empty() {
        telemetry
            .builder()
            .level(Level::Warn)
            .kind("auth.oauth.callback_failed")
            .category("auth")
            .message("oauth callback with empty provider name")
            .emit();
        return Err(shared::Error::InvalidInput("oauth provider name must not be empty".into()).into());
    }
    if code.trim().is_empty() {
        telemetry
            .builder()
            .level(Level::Warn)
            .kind("auth.oauth.callback_failed")
            .category("auth")
            .message(format!("oauth callback from {} with empty code", provider.name))
            .emit();
        return Err(shared::Error::InvalidInput("oauth callback code must not be empty".into()).into());
    }
    if state.trim().is_empty() {
        telemetry
            .builder()
            .level(Level::Warn)
            .kind("auth.oauth.callback_failed")
            .category("auth")
            .message(format!("oauth callback from {} with empty state", provider.name))
            .emit();
        return Err(shared::Error::InvalidInput("oauth callback state must not be empty".into()).into());
    }
    telemetry
        .builder()
        .level(Level::Info)
        .kind("auth.oauth.callback")
        .category("auth")
        .message(format!("oauth callback received from {}", provider.name))
        .emit();
    Ok(String::new())
}
