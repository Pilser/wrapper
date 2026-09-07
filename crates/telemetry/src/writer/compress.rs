//! Optional gzip compression of rotated files.

use std::path::Path;

/// Compress the given JSON file to a `.gz` sibling.
///
/// Currently a no-op that returns the input unchanged; wiring GzEncoder here
/// is a straightforward follow-up that leaves the retention/rotation logic
/// unaffected.
pub fn compress_gz(_input: &Path) -> anyhow::Result<Option<std::path::PathBuf>> {
    Ok(None)
}