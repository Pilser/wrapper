//! Admin live dashboard stream: `GET /api/admin/stream?tenant={id}[&token=]`
//! upgrades to WebSocket (staff Bearer header or `?token=` for browsers)
//! and pushes a JSON snapshot every 2s until the client goes away:
//! `{vm, category, template, heartbeat}` for one tenant. Real-time reads
//! only — no guest changes, no polling endpoints to hammer.

use futures_util::{SinkExt, StreamExt};

use super::AppState;
use crate::server::auth_ctx;

fn query_param(req: &hyper::Request<hyper::body::Incoming>, key: &str) -> Option<String> {
    req.uri().query().unwrap_or("").split('&').find_map(|pair| {
        let mut parts = pair.splitn(2, '=');
        if parts.next() == Some(key) {
            parts.next().map(|v| {
                v.replace("%3A", ":")
                    .replace("%2F", "/")
                    .replace("%20", " ")
            })
        } else {
            None
        }
    })
}

/// Snapshot builder shared by the stream ticks (pure reads).
pub async fn snapshot(state: &AppState, tenant_id: &str) -> serde_json::Value {
    let conn = state.gdb.conn().ok();
    let tenant = match &conn {
        Some(c) => db::global::tenants::get_by_id(c, tenant_id)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let (category, template) = match &tenant {
        Some(t) => {
            let cat = match &conn {
                Some(c) => db::global::categories::get_by_id(c, &t.category_id)
                    .await
                    .ok()
                    .flatten(),
                None => None,
            };
            let tpl = match (&conn, &cat) {
                (Some(c), Some(cat)) => {
                    db::global::vm_templates::get_by_id(c, &cat.vm_template_id)
                        .await
                        .ok()
                        .flatten()
                }
                _ => None,
            };
            (cat, tpl)
        }
        None => (None, None),
    };
    // VM reservation row (ip/tap) + fc socket mtime as boot approximation.
    let (ip, tap): (Option<String>, Option<String>) = match &conn {
        Some(c) => db::global::networks::all(c)
            .await
            .ok()
            .and_then(|rows| {
                rows.into_iter()
                    .find(|r| r.tenant_id == tenant_id)
                    .map(|r| (Some(r.ip_address), Some(r.tap_device)))
            })
            .unwrap_or((None, None)),
        None => (None, None),
    };
    let socket_mtime = std::fs::metadata(format!("/tmp/fc_{tenant_id}.sock"))
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok().map(|d| d.as_secs()));
    let heartbeat_age = state
        .activity
        .last(tenant_id)
        .map(|t| t.elapsed().as_secs());
    serde_json::json!({
        "tenant": tenant_id,
        "vm": {
            "status": tenant.as_ref().map(|t| t.status.as_str()),
            "ip": ip, "tap": tap,
            "uptime_secs": socket_mtime,
        },
        "category": category.as_ref().map(|c| serde_json::json!({
            "id": c.id, "name": c.name,
            "status": format!("{:?}", c.status),
            "template_id": c.vm_template_id,
            "max_tenants": c.max_tenants,
        })),
        "template": template.as_ref().map(|t| serde_json::json!({
            "id": t.id, "name": t.name, "vcpu_count": t.vcpu_count,
            "memory_mb": t.memory_mb, "status": format!("{:?}", t.status),
        })),
        "heartbeat_age_secs": heartbeat_age,
    })
}

type FullBody = http_body_util::Full<bytes::Bytes>;

/// Live telemetry feed: `GET /api/admin/events?level=&kind=&tenant=&token=`.
/// Same WS upgrade + staff auth as the dashboard stream. Each frame is one
/// telemetry event JSON; server-side filtered so browsers just render.
pub async fn admin_events(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<hyper::Response<FullBody>, hyper::Error> {
    // Staff auth: header first, ?token= for browsers.
    let (mut parts, body) = req.into_parts();
    if !parts.headers.contains_key(hyper::header::AUTHORIZATION) {
        if let Some(q) = parts
            .uri
            .query()
            .unwrap_or("")
            .split('&')
            .find_map(|pair| {
                let mut it = pair.splitn(2, '=');
                (it.next() == Some("token")).then(|| it.next().unwrap_or("").to_string())
            })
            .filter(|t| !t.is_empty())
        {
            if let Ok(v) = hyper::header::HeaderValue::from_str(&format!("Bearer {q}")) {
                parts.headers.insert(hyper::header::AUTHORIZATION, v);
            }
        }
    }
    let param = |key: &str| {
        parts
            .uri
            .query()
            .unwrap_or("")
            .split('&')
            .find_map(|pair| {
                let mut it = pair.splitn(2, '=');
                (it.next() == Some(key)).then(|| it.next().unwrap_or("").to_string())
            })
            .unwrap_or_default()
    };
    let req = hyper::Request::from_parts(parts, body);
    if crate::server::auth_ctx::require_admin(state, &req).await.is_err() {
        return Ok(plain(
            hyper::StatusCode::UNAUTHORIZED,
            r#"{"error":"admin authorization required"}"#,
        ));
    }
    let ws_key = req
        .headers()
        .get("sec-websocket-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let is_upgrade = req
        .headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    if !is_upgrade || ws_key.is_empty() {
        return Ok(plain(
            hyper::StatusCode::BAD_REQUEST,
            "websocket upgrade required",
        ));
    }
    let accept = tokio_tungstenite::tungstenite::handshake::derive_accept_key(
        ws_key.as_bytes(),
    );
    let mut filter = telemetry::BroadcastFilter::default();
    for lvl in param("level").split(',').filter(|s| !s.is_empty()) {
        filter.levels.push(lvl.to_string());
    }
    for t in param("tenant").split(',').filter(|s| !s.is_empty()) {
        filter.tenants.push(t.to_string());
    }
    for k in param("kind").split(',').filter(|s| !s.is_empty()) {
        filter.events.push(k.to_string());
    }
    let state = state.clone();
    tokio::spawn(async move {
        let upgraded = match hyper::upgrade::on(req).await {
            Ok(u) => u,
            Err(_) => return,
        };
        let mut ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
            hyper_util::rt::TokioIo::new(upgraded),
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        )
        .await;
        let (mut tx, _) = ws.split();
        let mut sub = state.telemetry.subscribe();
        sub.with_filter(filter);
        use tokio_tungstenite::tungstenite::Message as WsMsg;
        while let Some(ev) = sub.recv().await {
            let text =
                serde_json::to_string(&ev).unwrap_or_else(|_| "{}".to_string());
            if tx.send(WsMsg::Text(text.into())).await.is_err() {
                break;
            }
        }
    });
    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Accept", accept)
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .unwrap())
}

fn plain(status: hyper::StatusCode, text: &str) -> hyper::Response<FullBody> {
    hyper::Response::builder()
        .status(status)
        .body(http_body_util::Full::new(bytes::Bytes::from(
            text.to_string(),
        )))
        .unwrap()
}

pub async fn admin_stream(
    state: &AppState,
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<hyper::Response<FullBody>, hyper::Error> {
    // Staff auth: header first, ?token= for browsers (WS can't set headers).
    // Inject a query token as a header so the standard check applies.
    let (mut parts, body) = req.into_parts();
    if !parts.headers.contains_key(hyper::header::AUTHORIZATION) {
        if let Some(q) = parts
            .uri
            .query()
            .unwrap_or("")
            .split('&')
            .find_map(|pair| {
                let mut it = pair.splitn(2, '=');
                (it.next() == Some("token")).then(|| it.next().unwrap_or("").to_string())
            })
            .filter(|t| !t.is_empty())
        {
            if let Ok(v) = hyper::header::HeaderValue::from_str(&format!("Bearer {q}")) {
                parts.headers.insert(hyper::header::AUTHORIZATION, v);
            }
        }
    }
    let tenant_id = parts
        .uri
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|pair| {
            let mut it = pair.splitn(2, '=');
            (it.next() == Some("tenant")).then(|| it.next().unwrap_or("").to_string())
        })
        .unwrap_or_default();
    let req = hyper::Request::from_parts(parts, body);
    if auth_ctx::require_admin(state, &req).await.is_err() {
        return Ok(plain(
            hyper::StatusCode::UNAUTHORIZED,
            r#"{"error":"admin authorization required"}"#,
        ));
    }
    if tenant_id.trim().is_empty() {
        return Ok(plain(
            hyper::StatusCode::BAD_REQUEST,
            r#"{"error":"missing ?tenant=<id>"}"#,
        ));
    }
    // Browsers validate the accept key, so derive it before moving req.
    let ws_key = req
        .headers()
        .get("sec-websocket-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let is_upgrade = req
        .headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    if !is_upgrade || ws_key.is_empty() {
        return Ok(plain(
            hyper::StatusCode::BAD_REQUEST,
            "websocket upgrade required",
        ));
    }
    let accept = tokio_tungstenite::tungstenite::handshake::derive_accept_key(
        ws_key.as_bytes(),
    );
    let state = state.clone();
    tokio::spawn(async move {
        let upgraded = match hyper::upgrade::on(req).await {
            Ok(u) => u,
            Err(_) => return,
        };
        let mut ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
            hyper_util::rt::TokioIo::new(upgraded),
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        )
        .await;
        let (mut tx, _) = ws.split();
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
        // tenant=all streams the whole fleet (admin landing page).
        let fleet = tenant_id == "all";
        loop {
            tick.tick().await;
            let frame = if fleet {
                let ids: Vec<String> = match state.gdb.conn() {
                    Ok(c) => db::global::tenants::list(&c)
                        .await
                        .map(|rows| rows.into_iter().map(|t| t.id).collect())
                        .unwrap_or_default(),
                    Err(_) => Vec::new(),
                };
                let mut snaps = Vec::new();
                for id in ids {
                    snaps.push(snapshot(&state, &id).await);
                }
                serde_json::json!({"tenants": snaps}).to_string()
            } else {
                snapshot(&state, &tenant_id).await.to_string()
            };
            use tokio_tungstenite::tungstenite::Message as WsMsg;
            if tx.send(WsMsg::Text(frame.into())).await.is_err() {
                break;
            }
        }
    });
    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Accept", accept)
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .unwrap())
}
