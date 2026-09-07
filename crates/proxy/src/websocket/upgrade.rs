//! WebSocket upgrade detection stub.

use shared::Result;
use telemetry::{Level, Telemetry};

/// True when the request asks for a WebSocket upgrade.
pub fn is_upgrade_request(req: &hyper::Request<hyper::body::Incoming>) -> bool {
    let has_upgrade = req
        .headers()
        .get(hyper::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    let has_key = req.headers().contains_key("sec-websocket-key");
    has_upgrade && has_key
}

/// Log the upgrade attempt; returns whether it looks like a WS handshake.
pub async fn note_upgrade_attempt(
    telemetry: &Telemetry,
    req: &hyper::Request<hyper::body::Incoming>,
) -> Result<bool> {
    let is_ws = is_upgrade_request(req);
    telemetry
        .builder()
        .level(Level::Info)
        .kind("proxy.websocket.connected")
        .category("proxy")
        .source("proxy::websocket::upgrade")
        .message(format!(
            "websocket upgrade attempt path={} is_ws={is_ws}",
            req.uri().path()
        ))
        .emit();
    Ok(is_ws)
}
