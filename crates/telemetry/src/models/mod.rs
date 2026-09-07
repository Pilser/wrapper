//! JSON models mirroring the telemetry event schema.

pub mod error;
pub mod event;
pub mod metric;
pub mod request;
pub mod span;
pub mod state;

pub use error::ErrorPayload;
pub use event::Event;
pub use metric::MetricPayload;
pub use request::RequestPayload;
pub use span::{SpanPayload, SpanStatus};
pub use state::StatePayload;