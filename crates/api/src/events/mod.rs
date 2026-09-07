//! Plugin-visible event helpers: emit and subscribe.

pub mod emit;
pub mod subscribe;

pub use emit::{emit_error, emit_info};
pub use subscribe::subscribe;
