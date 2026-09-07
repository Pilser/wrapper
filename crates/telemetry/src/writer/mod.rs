//! Disk persistence of events as daily JSON files.

pub mod buffer;
pub mod compress;
pub mod file;
pub mod flush;
pub mod rotation;

pub use file::FileWriter;