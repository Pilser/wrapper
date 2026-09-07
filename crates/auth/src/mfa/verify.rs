//! MFA verification.

use telemetry::{Level, Telemetry};

pub fn verify_mfa(telemetry: &Telemetry, token: &str, expected: &str) -> anyhow::Result<bool> {
    let ok = token == expected;
    if !ok {
        telemetry.builder().level(Level::Warn).kind("auth.login.failed")
            .category("auth").message("mfa verification failed").emit();
    }
    Ok(ok)
}
