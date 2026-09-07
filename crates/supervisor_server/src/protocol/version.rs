use core::{Error, Result};

pub const PROTOCOL_VERSION: &str = "1.0";
pub const MIN_COMPATIBLE_VERSION: &str = "1.0";

pub fn current_version() -> &'static str {
    PROTOCOL_VERSION
}

/// Parse a `"<major>.<minor>"` version string.
pub fn parse_version(v: &str) -> Result<(u64, u64)> {
    let mut parts = v.trim().split('.');
    let major = parts
        .next()
        .unwrap_or("")
        .parse::<u64>()
        .map_err(|_| Error::InvalidInput(format!("bad protocol version `{v}`")))?;
    let minor = parts
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| Error::InvalidInput(format!("bad protocol version `{v}`")))?;
    Ok((major, minor))
}
