//! Read `config.toml` from disk, then parse and validate it.

use shared::{Error, Result};

use super::{parse::parse, validate::validate, Config};

/// Load, parse and validate the configuration file at `path`.
pub fn load(path: &str) -> Result<Config> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Error::Config(format!("cannot read {path}: {e}")))?;
    let cfg = parse(&text)?;
    validate(&cfg)?;
    Ok(cfg)
}
