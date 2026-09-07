//! Bandwidth measurement type.

use serde::{Deserialize, Serialize};

/// A bandwidth sample for an interface.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bandwidth {
    pub iface: String,
    pub rx_bytes_delta: u64,
    pub tx_bytes_delta: u64,
    pub bandwidth_rate_bytes: u64,
}
