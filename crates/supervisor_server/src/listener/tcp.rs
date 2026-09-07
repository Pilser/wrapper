use core::Result;
use std::sync::Arc;
use telemetry::{Level, Telemetry};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use super::authenticate::{authenticate, extract_token};
use super::rate_limit::RateLimiter;
use crate::mcp::handler;
use crate::protocol::message::Envelope;

/// Shared state for every accepted supervisor connection.
#[derive(Clone)]
pub struct ListenerState {
    pub telemetry: Telemetry,
    pub db: db::Db,
    pub auth: auth::AuthService,
}

/// Bind and serve JSON-over-TCP supervisor connections forever.
pub async fn serve(
    addr: &str,
    telemetry: Telemetry,
    db: db::Db,
    auth: auth::AuthService,
) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.listening")
        .category("supervisor")
        .message(format!("supervisor_server listening on {addr}"))
        .emit();
    let state = Arc::new(ListenerState {
        telemetry,
        db,
        auth,
    });
    loop {
        let (socket, peer) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let peer_str = peer.to_string();
            if let Err(e) = handle_connection(socket, peer_str.clone(), state).await {
                let _ = (e, peer_str);
            }
        });
    }
}

/// Handle one supervisor TCP connection: newline-delimited envelopes.
pub async fn handle_connection(
    stream: TcpStream,
    peer: String,
    state: Arc<ListenerState>,
) -> Result<()> {
    state
        .telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.connected")
        .category("supervisor")
        .message(format!("supervisor connected from {peer}"))
        .emit();
    let (reader_half, mut writer_half) = stream.into_split();
    let mut reader = BufReader::new(reader_half);
    let mut limiter = RateLimiter::new(600);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !limiter.allow(&peer) {
            let resp = Envelope::new(
                "error",
                "",
                serde_json::json!({"error": "rate_limited"}),
            );
            let out = resp.to_line()?;
            writer_half.write_all(out.as_bytes()).await?;
            writer_half.write_all(b"\n").await?;
            continue;
        }
        let response = handle_line(trimmed, &state).await;
        let out = response.to_line()?;
        writer_half.write_all(out.as_bytes()).await?;
        writer_half.write_all(b"\n").await?;
    }
    state
        .telemetry
        .builder()
        .level(Level::Info)
        .kind("supervisor.disconnected")
        .category("supervisor")
        .message(format!("supervisor disconnected {peer}"))
        .emit();
    Ok(())
}

async fn handle_line(line: &str, state: &ListenerState) -> Envelope {
    let envelope = match Envelope::from_line(line) {
        Ok(e) => e,
        Err(e) => {
            return Envelope::new(
                "error",
                "",
                serde_json::json!({"error": e.to_string()}),
            );
        }
    };
    let token = extract_token(&envelope.payload);
    if let Err(e) = authenticate(
        &envelope.tenant_id,
        token.as_deref(),
        &state.auth,
        &state.telemetry,
    )
    .await
    {
        return envelope.respond("error", serde_json::json!({"error": e.to_string()}));
    }
    match handler::dispatch(&envelope, &state.db, &state.telemetry).await {
        Ok(payload) => envelope.respond("response", payload),
        Err(e) => envelope.respond("error", serde_json::json!({"error": e.to_string()})),
    }
}
