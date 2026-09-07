//! Folder watching for plugin hot-reload.

pub mod event;
pub mod folder;
pub mod scan;

pub use folder::FolderWatcher;
