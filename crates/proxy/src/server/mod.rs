//! TCP/TLS listeners plus graceful shutdown for the proxy server.

pub mod shutdown;
pub mod tcp;
pub mod tls;

pub use shutdown::GracefulShutdown;
pub use tcp::TcpAcceptor;
pub use tls::TlsConfig;

use shared::{Error as WrapperError, Result};
use telemetry::{Level, Telemetry};

/// Hyper-based reverse-proxy server.
///
/// Serves plain TCP (see [`TlsConfig`]: no in-process TLS termination).
/// Each accepted connection is served by a shared [`crate::http::HttpHandler`]
/// that forwards to the tenant's VM; use [`ProxyServer::with_router`] /
/// [`ProxyServer::with_vm_lookup`] to thread routing and DB-backed upstream
/// resolution through. Stops when `shutdown` fires.
#[derive(Clone)]
pub struct ProxyServer {
    bind: String,
    telemetry: Telemetry,
    handler: crate::http::HttpHandler,
}

impl std::fmt::Debug for ProxyServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyServer")
            .field("bind", &self.bind)
            .field("handler", &self.handler)
            .finish()
    }
}

impl ProxyServer {
    pub fn new(
        bind: impl Into<String>,
        telemetry: Telemetry,
        runtime: vm::runtime::VmRuntime,
    ) -> Self {
        let handler = crate::http::HttpHandler::new(telemetry.clone(), runtime.clone());
        Self {
            bind: bind.into(),
            telemetry,
            handler,
        }
    }

    pub fn bind_addr(&self) -> &str {
        &self.bind
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    /// Override the router used to extract tenant ids from requests.
    pub fn with_router(mut self, router: crate::routing::Router) -> Self {
        self.handler = self.handler.with_router(router);
        self
    }

    /// Override VM address resolution (e.g. a DB-backed [`crate::routing::VmLookup`]).
    pub fn with_vm_lookup(mut self, lookup: crate::routing::VmLookup) -> Self {
        self.handler = self.handler.with_lookup(lookup);
        self
    }

    /// Override the wake-on-request VM runtime.
    pub fn with_runtime(mut self, runtime: vm::runtime::VmRuntime) -> Self {
        self.handler = self.handler.with_runtime(runtime);
        self
    }

    /// Override the full request handler.
    pub fn with_handler(mut self, handler: crate::http::HttpHandler) -> Self {
        self.handler = handler;
        self
    }

    /// Bind and serve until `shutdown` fires, forwarding every connection to
    /// its tenant's upstream VM via the shared [`crate::http::HttpHandler`].
    pub async fn start(&self, mut shutdown: GracefulShutdown) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(&self.bind).await?;
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.received")
            .category("proxy")
            .source("proxy::server")
            .message(format!("proxy listening on {}", self.bind))
            .emit();

        loop {
            tokio::select! {
                _ = shutdown.wait() => {
                    self.telemetry
                        .builder()
                        .level(Level::Info)
                        .kind("proxy.request.completed")
                        .category("proxy")
                        .source("proxy::server")
                        .message("proxy shutdown requested")
                        .emit();
                    break;
                }
                accepted = listener.accept() => {
                    let (stream, _peer) = accepted.map_err(WrapperError::from)?;
                    let telemetry = self.telemetry.clone();
                    let handler = self.handler.clone();
                    tokio::spawn(async move {
                        let io = hyper_util::rt::TokioIo::new(stream);
                        let service = hyper::service::service_fn(move |req| {
                            let handler = handler.clone();
                            async move { handler.handle(req).await }
                        });
                        let builder = hyper_util::server::conn::auto::Builder::new(
                            hyper_util::rt::TokioExecutor::new(),
                        );
                        let conn = builder.serve_connection_with_upgrades(io, service);
                        if let Err(err) = conn.await {
                            telemetry
                                .builder()
                                .level(Level::Error)
                                .kind("proxy.request.failed")
                                .category("proxy")
                                .source("proxy::server")
                                .message(format!("connection error: {err}"))
                                .emit();
                        }
                    });
                }
            }
        }
        Ok(())
    }
}
