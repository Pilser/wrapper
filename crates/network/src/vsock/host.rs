//! Host side of a vsock channel: a handle that maps a CID to a port.

use std::collections::HashMap;

use anyhow::Result;


/// Host-side vsock registry (guest CID -> listening socket path).
#[derive(Debug, Default, Clone)]
pub struct VsockHost {
    sockets: HashMap<u32, String>,
}

impl VsockHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, cid: u32, socket_path: impl Into<String>) -> Result<()> {
        self.sockets.insert(cid, socket_path.into());
        Ok(())
    }

    pub fn socket_for(&self, cid: u32) -> Option<&str> {
        self.sockets.get(&cid).map(String::as_str)
    }
}
