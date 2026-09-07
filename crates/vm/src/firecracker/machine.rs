//! Machine wrapper: keeps a client + assigned id.

use super::client::FirecrackerClient;

/// A booted microVM instance handle.
pub struct Machine {
    pub vm_id: String,
    pub client: FirecrackerClient,
}

impl Machine {
    pub fn new(vm_id: impl Into<String>, socket: impl Into<String>) -> Self {
        Self { vm_id: vm_id.into(), client: FirecrackerClient::new(socket) }
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        let info = self.client.get("v2/actions/describe").await?;
        let _ = info;
        Ok(())
    }
}
