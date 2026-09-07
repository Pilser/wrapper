//! Structured error payload attached to error events.

use serde::{Serialize, Deserialize};

/// Error details carried by a `level: error` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Coarse error kind, e.g. `ResourceExhausted`.
    pub kind: String,
    /// Human readable message.
    pub message: String,
    /// Optional call-stack style trace.
    pub stack: Option<String>,
    /// Free-form context.
    pub context: std::collections::HashMap<String, serde_json::Value>,
}

impl ErrorPayload {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            stack: None,
            context: Default::default(),
        }
    }
}