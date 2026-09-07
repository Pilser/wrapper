//! Filesystem helpers — atomic writes and JSON persistence.

use std::path::Path;

use crate::Result;

/// Atomically write `bytes` to `path` via a temporary sibling + rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(crate::Error::Io)?;
    }
    let tmp = temp_name(path);
    std::fs::write(&tmp, bytes).map_err(crate::Error::Io)?;
    std::fs::rename(&tmp, path).map_err(crate::Error::Io)?;
    Ok(())
}

/// Read a JSON value from disk.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(crate::Error::Io)?;
    let value = serde_json::from_slice(&bytes).map_err(crate::Error::Json)?;
    Ok(Some(value))
}

/// Write a value as pretty JSON to disk (atomically).
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(crate::Error::Json)?;
    write_atomic(path, &bytes)
}

/// Ensure a directory tree exists.
pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(crate::Error::Io)
}

/// Recursively remove a directory, ignoring "not found" errors.
pub fn remove_dir_all(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(crate::Error::Io)?;
    }
    Ok(())
}

fn temp_name(path: &Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    std::path::PathBuf::from(name)
}