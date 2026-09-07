//! Watch event descriptions.

/// What changed for a plugin file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEvent {
    Added,
    Modified,
    Removed,
}
