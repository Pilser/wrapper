use core::{Error, Result};

use super::version::{MIN_COMPATIBLE_VERSION, PROTOCOL_VERSION, parse_version};

/// Same-major compatibility check.
pub fn is_compatible(client_version: &str) -> bool {
    match (
        parse_version(client_version),
        parse_version(PROTOCOL_VERSION),
    ) {
        (Ok((client_major, _)), Ok((server_major, _))) => client_major == server_major,
        _ => false,
    }
}

pub fn check_compatible(client_version: &str) -> Result<()> {
    if client_version.trim().is_empty() {
        return Ok(());
    }
    if is_compatible(client_version) {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!(
            "incompatible protocol version `{client_version}`, server is {PROTOCOL_VERSION}"
        )))
    }
}

pub fn min_compatible() -> &'static str {
    MIN_COMPATIBLE_VERSION
}
