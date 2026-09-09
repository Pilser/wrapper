//! `proxy` — hyper-based reverse proxy: HTTP, WebSocket, routing, middleware,
//! header transform and upstream connection pooling.
//!
//! Plugin API: [`ProxyHooks`] lets plugins register `on_request`,
//! `on_response` and `on_websocket_connect` callbacks.

pub mod http;
pub mod middleware;
pub mod pool;
pub mod routing;
pub mod server;
pub mod transform;
pub mod websocket;

pub use http::HttpHandler;
pub use middleware::{AuthMiddleware, CorsMiddleware, LoggingMiddleware, MiddlewareChain, RateLimiter};
pub use pool::{ConnectionPool, HealthChecker, PooledConn};
pub use routing::{ActivityMap, FallbackHandler, Router, TenantRoute, VmLookup};
pub use server::{GracefulShutdown, ProxyServer, TcpAcceptor, TlsConfig};
pub use transform::{RequestTransform, ResponseTransform};
pub use websocket::{PingTracker, WsFrame, WsHandler, WsStream};

use std::sync::{Arc, Mutex};

/// Simple synchronous callback invoked with a short info string.
pub type HookCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// Minimal plugin hook registry for the proxy.
///
/// Plugins register callbacks; the proxy fires them at request/response/
/// websocket-connect points. Kept synchronous and infallible so a plugin
/// can never break the hot path.
#[derive(Clone, Default)]
pub struct ProxyHooks {
    on_request: Arc<Mutex<Vec<HookCallback>>>,
    on_response: Arc<Mutex<Vec<HookCallback>>>,
    on_websocket_connect: Arc<Mutex<Vec<HookCallback>>>,
}

impl std::fmt::Debug for ProxyHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyHooks").finish()
    }
}

impl ProxyHooks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_on_request(&self, cb: HookCallback) {
        if let Ok(mut guard) = self.on_request.lock() {
            guard.push(cb);
        }
    }

    pub fn register_on_response(&self, cb: HookCallback) {
        if let Ok(mut guard) = self.on_response.lock() {
            guard.push(cb);
        }
    }

    pub fn register_on_websocket_connect(&self, cb: HookCallback) {
        if let Ok(mut guard) = self.on_websocket_connect.lock() {
            guard.push(cb);
        }
    }

    pub fn fire_on_request(&self, info: &str) {
        if let Ok(guard) = self.on_request.lock() {
            for cb in guard.iter() {
                cb(info);
            }
        }
    }

    pub fn fire_on_response(&self, info: &str) {
        if let Ok(guard) = self.on_response.lock() {
            for cb in guard.iter() {
                cb(info);
            }
        }
    }

    pub fn fire_on_websocket_connect(&self, info: &str) {
        if let Ok(guard) = self.on_websocket_connect.lock() {
            for cb in guard.iter() {
                cb(info);
            }
        }
    }
}
