//! Polls the plugin folder and reports changed plugins.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Watches a folder, returning plugin filenames whose mtime changed.
#[derive(Debug)]
pub struct FolderWatcher {
    folder: PathBuf,
    pub(crate) seen: Mutex<HashMap<PathBuf, u64>>,
}

impl FolderWatcher {
    pub fn new(folder: PathBuf) -> Self {
        Self {
            folder,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// Return the `.so` files currently in the folder.
    pub fn current(&self) -> anyhow::Result<Vec<PathBuf>> {
        let out: Vec<PathBuf> = self.folder
            .read_dir()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "so").unwrap_or(false))
            .collect();
        Ok(out)
    }

    /// Scan for (re)added plugins: returns paths that are new or changed.
    pub fn scan(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        let current = self.current()?;
        let mut seen = self.seen.lock().unwrap();
        for path in current {
            let mtime = std::fs::metadata(&path)?.modified().map(|m| m.elapsed().unwrap_or_default().as_nanos() as u64).unwrap_or(0);
            if seen.get(&path).copied() != Some(mtime) {
                seen.insert(path.clone(), mtime);
                out.push(path);
            }
        }
        Ok(out)
    }
}
