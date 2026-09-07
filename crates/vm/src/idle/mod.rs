//! Idle-VM tracking and timeout action.

pub mod action;
pub mod timeout;
pub mod tracker;

pub use action::IdleAction;
pub use tracker::IdleTracker;
