//! VM manager, registry, pool and category routing.

pub mod category;
pub mod pool;
pub mod registry;
pub mod vm_manager;

pub use registry::VmRegistry;
pub use vm_manager::VmManager;
