//! Telemetry event categories shared across the wrapper.

use serde::{Deserialize, Serialize};

/// The event category is the second segment of a telemetry `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventCategory {
    System,
    Vm,
    Network,
    Proxy,
    Auth,
    Supervisor,
    Admin,
    Database,
    Plugin,
    Metrics,
    Performance,
}

impl EventCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventCategory::System => "system",
            EventCategory::Vm => "vm",
            EventCategory::Network => "network",
            EventCategory::Proxy => "proxy",
            EventCategory::Auth => "auth",
            EventCategory::Supervisor => "supervisor",
            EventCategory::Admin => "admin",
            EventCategory::Database => "db",
            EventCategory::Plugin => "plugin",
            EventCategory::Metrics => "metrics",
            EventCategory::Performance => "performance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Started,
    Completed,
    Failed,
    Received,
    Sent,
    Created,
    Deleted,
    Updated,
    Connected,
    Disconnected,
    Emitted,
    Allocated,
    Released,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Started => "started",
            EventType::Completed => "completed",
            EventType::Failed => "failed",
            EventType::Received => "received",
            EventType::Sent => "sent",
            EventType::Created => "created",
            EventType::Deleted => "deleted",
            EventType::Updated => "updated",
            EventType::Connected => "connected",
            EventType::Disconnected => "disconnected",
            EventType::Emitted => "emitted",
            EventType::Allocated => "allocated",
            EventType::Released => "released",
        }
    }
}