//! Event construction: builder, emitters and levels.

pub mod builder;
pub mod emit;
pub mod level;
pub mod types;

pub use builder::EventBuilder;
pub use emit::{emit_event, span_timer};
pub use level::Level;
pub use types::EventId;