//! The tenant IP pool.

pub mod allocate;
pub mod conflict;
pub mod pool;
pub mod release;

pub use core::IpAllocation;
pub use pool::IpPool;
