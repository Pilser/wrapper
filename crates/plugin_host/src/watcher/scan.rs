//! Scan helpers (full rescan triggered by watcher events).

use std::path::Path;

use super::folder::FolderWatcher;

impl FolderWatcher {
    /// Force a full re-scan (ignores cached mtimes).
    pub fn rescan(&self) -> anyhow::Result<Vec<std::path::PathBuf>> {
        self.seen.lock().unwrap().clear();
        self.scan()
    }
}
