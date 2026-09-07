use shared::{Error, Result};

use super::tools::is_known_tool;

/// A supervisor may only invoke known tools for a valid tenant.
pub fn can_call(tenant_id: &str, tool: &str) -> Result<()> {
    crate::validate_tenant_id(tenant_id)?;
    if tool.trim().is_empty() {
        return Err(Error::InvalidInput("missing tool name".to_string()));
    }
    if !is_known_tool(tool) {
        return Err(Error::NotFound(format!("unknown tool {tool}")));
    }
    Ok(())
}

/// The envelope tenant must match any tenant claimed inside the payload.
pub fn check_tenant_match(envelope_tenant: &str, claimed: Option<&str>) -> Result<()> {
    crate::validate_tenant_id(envelope_tenant)?;
    if let Some(c) = claimed {
        if c != envelope_tenant {
            return Err(Error::Unauthorized("tenant mismatch".to_string()));
        }
    }
    Ok(())
}
