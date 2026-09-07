//! Hook points a plugin can subscribe to.

use serde::{Deserialize, Serialize};

/// Lifecycle / event hooks the wrapper dispatches to plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
    /// A telemetry event was emitted.
    OnEventEmitted,
    /// Telemetry file flushed to disk.
    OnFileFlushed,
    /// A telemetry event was broadcast to dashboards.
    OnBroadcastSent,
    /// A metric was recorded.
    OnMetricRecorded,
    /// A database query was started / completed.
    OnDbQuery,
    /// A VM lifecycle operation fired.
    OnVmLifecycle,
    /// A proxy request round-trip started / completed.
    OnProxyRequest,
    /// An authentication event fired.
    OnAuthEvent,
    /// A plugin itself was loaded.
    OnPluginLoaded,
    /// A plugin was unloaded.
    OnPluginUnloaded,
    /// Shared catch-all for extension events.
    OnPluginCustom,
}

impl HookPoint {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookPoint::OnEventEmitted => "telemetry.event_emitted",
            HookPoint::OnFileFlushed => "telemetry.file_flushed",
            HookPoint::OnBroadcastSent => "telemetry.broadcast_sent",
            HookPoint::OnMetricRecorded => "telemetry.metric_recorded",
            HookPoint::OnDbQuery => "db.query",
            HookPoint::OnVmLifecycle => "vm.lifecycle",
            HookPoint::OnProxyRequest => "proxy.request",
            HookPoint::OnAuthEvent => "auth.event",
            HookPoint::OnPluginLoaded => "plugin.loaded",
            HookPoint::OnPluginUnloaded => "plugin.unloaded",
            HookPoint::OnPluginCustom => "plugin.custom",
        }
    }
}