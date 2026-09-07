//! Guest side of a vsock channel.

#[derive(Debug, Clone)]
pub struct VsockGuest {
    pub host_cid: u32,
    pub port: u16,
}

impl VsockGuest {
    pub fn new(host_cid: u32, port: u16) -> Self {
        Self { host_cid, port }
    }

    pub fn endpoint(&self) -> String {
        format!("cid:{}:port:{}", self.host_cid, self.port)
    }
}
