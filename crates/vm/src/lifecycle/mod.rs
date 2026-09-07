//! VM lifecycle operations.

pub mod boot;
pub mod migrate;
pub mod reboot;
pub mod restore;
pub mod shutdown;
pub mod snapshot;

pub use boot::boot_vm;
pub use migrate::migrate_vm;
pub use reboot::reboot_vm;
pub use restore::restore_vm;
pub use shutdown::shutdown_vm;
pub use snapshot::snapshot_vm;
