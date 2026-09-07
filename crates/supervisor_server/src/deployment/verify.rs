use core::{DeploymentManifest, Error, Result};
use telemetry::{Level, Telemetry};

/// Compare two `sha256:<hex>` checksum strings in constant time.
///
/// NOTE: the workspace has no `sha2` (or other hashing) dependency, so this
/// crate cannot recompute digests from content. Instead it compares a
/// provided checksum against the expected one with a constant-time byte
/// comparison over the normalized hex part (a `sha256:` prefix is accepted
/// and optional; hex is case-insensitive). Empty checksums are rejected —
/// verification never silently passes.
pub fn verify_checksum(provided: &str, expected: &str) -> Result<()> {
    if provided.trim().is_empty() || expected.trim().is_empty() {
        return Err(Error::InvalidInput("missing checksum".to_string()));
    }
    fn normalize(s: &str) -> String {
        s.trim()
            .strip_prefix("sha256:")
            .unwrap_or(s.trim())
            .to_ascii_lowercase()
    }
    fn valid_hex(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
    }
    let a = normalize(provided);
    let b = normalize(expected);
    if !valid_hex(&a) || !valid_hex(&b) {
        return Err(Error::InvalidInput(
            "bad checksum format, expected `sha256:<hex>`".to_string(),
        ));
    }
    if a.len() != b.len() {
        return Err(Error::InvalidInput("checksum mismatch".to_string()));
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    if diff == 0 {
        Ok(())
    } else {
        Err(Error::InvalidInput("checksum mismatch".to_string()))
    }
}

/// Validate a deployment manifest: tenant/category present and every
/// artifact carrying a non-empty, well-formed `sha256:<hex>` checksum.
pub fn verify_package(pkg: &DeploymentManifest) -> Result<()> {
    if pkg.tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "deployment missing `tenant_id`".to_string(),
        ));
    }
    if pkg.category.trim().is_empty() {
        return Err(Error::InvalidInput(
            "deployment missing `category`".to_string(),
        ));
    }
    for artifact in &pkg.artifacts {
        if artifact.name.trim().is_empty() || artifact.path.trim().is_empty() {
            return Err(Error::InvalidInput(
                "deployment artifact missing `name`/`path`".to_string(),
            ));
        }
        if artifact.checksum.trim().is_empty() {
            return Err(Error::InvalidInput(format!(
                "deployment artifact `{}` missing checksum",
                artifact.name
            )));
        }
        // Well-formedness only (no second checksum to compare against here).
        verify_checksum(&artifact.checksum, &artifact.checksum)?;
    }
    Telemetry::noop();
    Ok(())
}

/// Emit helper so `receive` can log verification outcomes.
pub fn emit_verified(telemetry: &Telemetry, tenant_id: &str, category: &str) {
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.deployment.acknowledged")
        .category("supervisor")
        .message(format!(
            "deployment {category} verified tenant {tenant_id}"
        ))
        .emit();
}
