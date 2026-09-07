//! Graceful shutdown coordination.

pub mod graceful;
pub mod signal;

pub use graceful::GracefulShutdown;
pub use signal::wait_for_signal;
