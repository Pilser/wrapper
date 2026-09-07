//! Lifecycle hook dispatch.

pub mod dispatch;
pub mod error;
pub mod filter;
pub mod priority;

pub use dispatch::{HookCallback, HookDispatcher};
