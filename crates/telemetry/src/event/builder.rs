//! Fluent builder for events.

use serde_json::{Map, Value};

use super::Level;
use crate::models::{
    ErrorPayload, Event, MetricPayload, RequestPayload, SpanPayload, StatePayload,
};
use crate::Telemetry;

/// Builder used to assemble and then emit an [`Event`].
#[derive(Clone)]
pub struct EventBuilder {
    pub(crate) telemetry: Telemetry,
    pub(crate) event: Event,
}

impl EventBuilder {
    pub fn new(telemetry: Telemetry) -> Self {
        Self {
            telemetry,
            event: Event::new(),
        }
    }

    pub fn level(mut self, level: Level) -> Self {
        self.event.level = level;
        self
    }

    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.event.kind = kind.into();
        self
    }

    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.event.category = category.into();
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.event.source = source.into();
        self
    }

    pub fn tenant_id(mut self, v: Option<String>) -> Self {
        self.event.tenant_id = v;
        self
    }

    pub fn vm_id(mut self, v: Option<String>) -> Self {
        self.event.vm_id = v;
        self
    }

    pub fn session_id(mut self, v: Option<String>) -> Self {
        self.event.session_id = v;
        self
    }

    pub fn trace_id(mut self, v: Option<String>) -> Self {
        self.event.trace_id = v;
        self
    }

    pub fn span_id(mut self, v: Option<String>) -> Self {
        self.event.span_id = v;
        self
    }

    pub fn message(mut self, v: impl Into<String>) -> Self {
        self.event.message = Some(v.into());
        self
    }

    pub fn data(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.event.data.insert(key.into(), value.into());
        self
    }

    pub fn data_value(mut self, map: Map<String, Value>) -> Self {
        self.event.data = map;
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.event.tags.push(tag.into());
        self
    }

    pub fn tenant_category(mut self, v: Option<String>) -> Self {
        self.event.tenant_category = v;
        self
    }

    pub fn error(mut self, p: ErrorPayload) -> Self {
        self.event.error = Some(p);
        self
    }

    pub fn metric(mut self, p: MetricPayload) -> Self {
        self.event.metric = Some(p);
        self
    }

    pub fn request(mut self, p: RequestPayload) -> Self {
        self.event.request = Some(p);
        self
    }

    pub fn state(mut self, p: StatePayload) -> Self {
        self.event.state = Some(p);
        self
    }

    pub fn span(mut self, p: SpanPayload) -> Self {
        self.event.span = Some(p);
        self
    }

    pub fn build(self) -> Event {
        self.event
    }

    /// Emit immediately.
    pub fn emit(self) {
        self.telemetry.emit(self.event);
    }
}