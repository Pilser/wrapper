//! Errors produced while dispatching hooks.

use std::fmt;

#[derive(Debug)]
pub struct HookError(pub String);

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hook error: {}", self.0)
    }
}

impl std::error::Error for HookError {}
