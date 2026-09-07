//! Access checks for plugin calls.

use super::ApiContext;
use core::{Error, Result};

/// Stub gate: rejects empty resources; otherwise allows the call.
pub fn check_access(ctx: &ApiContext, resource: &str) -> Result<()> {
    if resource.trim().is_empty() {
        return Err(Error::InvalidInput(
            "resource must not be empty".into(),
        ));
    }
    let _ = ctx;
    Ok(())
}

/// Whether this context is scoped to a tenant.
pub fn is_tenant_scoped(ctx: &ApiContext) -> bool {
    ctx.tenant_id.is_some()
}
