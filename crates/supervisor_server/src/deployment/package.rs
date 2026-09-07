use shared::{DeploymentManifest, Error, Result};
use serde_json::Value;

/// Wire format version of the deployment package exchange.
pub const VERSION: &str = "1.0";

/// Default port of the supervisor-side deployment listener.
pub const DEFAULT_DEPLOY_PORT: u16 = 8091;

/// Parse an inbound deployment manifest from an envelope payload.
///
/// Accepts the bare [`DeploymentManifest`] JSON or the transport wrapper
/// `{"op", "data", "token"}` with the manifest under `data`.
pub fn parse_package(payload: &Value) -> Result<DeploymentManifest> {
    let body = payload
        .get("data")
        .filter(|v| v.is_object())
        .unwrap_or(payload);
    let manifest: DeploymentManifest = serde_json::from_value(body.clone())?;
    if manifest.tenant_id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "deployment missing `tenant_id`".to_string(),
        ));
    }
    if manifest.category.trim().is_empty() {
        return Err(Error::InvalidInput(
            "deployment missing `category`".to_string(),
        ));
    }
    Ok(manifest)
}

/// Push a deployment manifest to the supervisor's deployment listener.
///
/// Connects to `vm_addr:port` (`port = None` selects
/// [`DEFAULT_DEPLOY_PORT`]), sends the manifest as one NDJSON
/// `Envelope{type: "deployment.push", ...}` line and reads back a single
/// ack line.
pub async fn send_package(
    manifest: &DeploymentManifest,
    vm_addr: &str,
    port: Option<u16>,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    let port = port.unwrap_or(DEFAULT_DEPLOY_PORT);
    let addr = format!("{vm_addr}:{port}");
    let envelope = crate::protocol::message::Envelope::new(
        "deployment.push",
        manifest.tenant_id.clone(),
        serde_json::to_value(manifest).map_err(Error::from)?,
    );
    let line = envelope.to_line()?;
    let stream = TcpStream::connect(&addr).await?;
    let (reader_half, mut writer_half) = stream.into_split();
    writer_half.write_all(line.as_bytes()).await?;
    writer_half.write_all(b"\n").await?;
    let mut reader = BufReader::new(reader_half);
    let mut ack = String::new();
    reader.read_line(&mut ack).await?;
    let _ack: crate::protocol::message::Envelope =
        serde_json::from_str(ack.trim()).map_err(Error::from)?;
    Ok(())
}
