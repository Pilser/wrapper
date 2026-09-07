//! vsock channel configuration.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VsockConfig {
    pub cid: u32,
    pub port: u16,
}

impl VsockConfig {
    pub fn new(cid: u32, port: u16) -> Self {
        Self { cid, port }
    }
}
