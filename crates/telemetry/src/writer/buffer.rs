//! In-memory event buffer.

use crate::models::Event;

/// A bounded in-memory collection of events awaiting a flush.
#[derive(Debug, Default)]
pub struct FloatBuffer {
    events: Vec<Event>,
}

impl FloatBuffer {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, ev: Event) {
        self.events.push(ev);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Take all buffered events, leaving the buffer empty.
    pub fn drain(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}