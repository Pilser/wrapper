//! Hyper HTTP reverse-proxy handler: middleware, transforms, forward.
//!
//! Flow per request: run the [`MiddlewareChain`], apply
//! [`RequestTransform`] + hop-by-hop [`sanitize`](crate::transform::sanitize),
//! route the tenant, resolve the upstream via [`VmLookup`], forward the
//! request with the hyper-util legacy client, then apply
//! [`ResponseTransform`] to the upstream response before returning it.

use bytes::Bytes;
use shared::{Error as WrapperError, Result};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use telemetry::{Level, Telemetry};

/// Shared upstream HTTP client (hyper 1 + hyper-util legacy, pooled).
type UpstreamClient = Client<HttpConnector, Full<Bytes>>;

fn upstream_client() -> UpstreamClient {
    Client::builder(hyper_util::rt::TokioExecutor::new()).build_http()
}

/// Serves proxied HTTP requests by forwarding them to the tenant's VM.
#[derive(Clone)]
pub struct HttpHandler {
    telemetry: Telemetry,
    router: crate::routing::Router,
    lookup: crate::routing::VmLookup,
    runtime: vm::runtime::VmRuntime,
    middleware: crate::middleware::MiddlewareChain,
    request_transform: crate::transform::RequestTransform,
    response_transform: crate::transform::ResponseTransform,
    client: UpstreamClient,
    /// Optional activity map touched per forwarded request (idle reaper).
    activity: Option<crate::routing::ActivityMap>,
}

impl std::fmt::Debug for HttpHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpHandler")
            .field("router", &self.router)
            .field("lookup", &self.lookup)
            .field("middleware", &self.middleware)
            .field("request_transform", &self.request_transform)
            .field("response_transform", &self.response_transform)
            .finish()
    }
}

impl HttpHandler {
    pub fn new(telemetry: Telemetry, runtime: vm::runtime::VmRuntime) -> Self {
        Self {
            router: crate::routing::Router::new(
                crate::routing::vm_lookup::DEFAULT_TARGET,
                telemetry.clone(),
            ),
            lookup: crate::routing::VmLookup::new(telemetry.clone()),
            runtime,
            middleware: crate::middleware::MiddlewareChain::new(telemetry.clone()),
            request_transform: crate::transform::RequestTransform::new(),
            response_transform: crate::transform::ResponseTransform::new(),
            client: upstream_client(),
            activity: None,
            telemetry,
        }
    }

    pub fn with_router(mut self, router: crate::routing::Router) -> Self {
        self.router = router;
        self
    }

    pub fn with_lookup(mut self, lookup: crate::routing::VmLookup) -> Self {
        self.lookup = lookup;
        self
    }

    pub fn with_runtime(mut self, runtime: vm::runtime::VmRuntime) -> Self {
        self.runtime = runtime;
        self
    }

    pub fn with_middleware(mut self, middleware: crate::middleware::MiddlewareChain) -> Self {
        self.middleware = middleware;
        self
    }

    pub fn with_request_transform(
        mut self,
        transform: crate::transform::RequestTransform,
    ) -> Self {
        self.request_transform = transform;
        self
    }

    pub fn with_response_transform(
        mut self,
        transform: crate::transform::ResponseTransform,
    ) -> Self {
        self.response_transform = transform;
        self
    }

    /// Record per-tenant activity on every forwarded request so the idle
    /// reaper can sleep quiet VMs.
    pub fn with_activity(mut self, activity: crate::routing::ActivityMap) -> Self {
        self.activity = Some(activity);
        self
    }

    pub async fn handle(
        &self,
        mut req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<Full<Bytes>>> {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.received")
            .category("proxy")
            .source("proxy::http::handler")
            .message(format!("HTTP request received {method} {path}"))
            .emit();

        if let Err(err) = self.middleware.run(&req).await {
            self.telemetry
                .builder()
                .level(Level::Error)
                .kind("proxy.request.failed")
                .category("proxy")
                .source("proxy::http::handler")
                .message(format!("middleware rejected {method} {path}: {err}"))
                .emit();
            let status = match &err {
                WrapperError::Unauthorized(_) => hyper::StatusCode::FORBIDDEN,
                _ => hyper::StatusCode::INTERNAL_SERVER_ERROR,
            };
            return Ok(crate::http::body::text_response(
                status,
                &err.to_string(),
            ));
        }

        self.request_transform.apply(&mut req)?;
        crate::transform::sanitize(req.headers_mut());

        let tenant_id = self.router.route_with_telemetry(&req).await?;

        // Resolve the tenant's VM address. If the tenant has no running VM
        // (lookup returns the default target) wake one up on demand. A
        // recorded address is PROBED first: a dead VM (crashed, reaped)
        // must re-wake, never black-hole into a stale IP.
        let upstream_addr = self.lookup.lookup(&tenant_id).await?;
        let upstream_addr = if upstream_addr == self.lookup.default_target() {
            self.wake(&tenant_id).await?
        } else if probe_upstream(&upstream_addr).await {
            upstream_addr
        } else {
            self.telemetry
                .builder()
                .level(telemetry::Level::Warn)
                .kind("proxy.vm.unreachable")
                .category("proxy")
                .source("proxy::http::handler")
                .message(format!(
                    "recorded VM {upstream_addr} for {tenant_id} unreachable, re-waking"
                ))
                .emit();
            self.wake(&tenant_id).await?
        };

        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.forwarded")
            .category("proxy")
            .source("proxy::http::handler")
            .message(format!(
                "forwarding {method} {path} tenant={tenant_id} to {upstream_addr}"
            ))
            .emit();

        if let Some(activity) = &self.activity {
            activity.touch(&tenant_id);
        }

        let mut res = self.forward(&upstream_addr, &tenant_id, req).await?;
        self.response_transform.apply(&mut res)?;
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.request.completed")
            .category("proxy")
            .source("proxy::http::handler")
            .message(format!(
                "HTTP request completed {method} {path} status={}",
                res.status()
            ))
            .emit();
        Ok(res)
    }

    /// Boot (or re-boot) the tenant's VM; returns its `IP:port`.
    async fn wake(&self, tenant_id: &str) -> Result<String> {
        self.runtime.ensure_running(tenant_id).await.map_err(|e| {
            WrapperError::Proxy(format!("wake VM for {tenant_id}: {e}"))
        })
    }

    /// Forward one request to `upstream_addr`, copying method/uri/headers/body.    ///
    /// PUBLIC-LINK CONTRACT (deployment/PUBLIC-LINK-ROUTING.txt): the
    /// `/tenant_<id>` prefix is STRIPPED so zw always sees clean app paths,
    /// and `x-tenant-id` is always set on the upstream request.
    async fn forward(
        &self,
        upstream_addr: &str,
        tenant_id: &str,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<Full<Bytes>>> {
        let (mut parts, body) = req.into_parts();
        let path_and_query = parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        let clean_path = strip_tenant_prefix(&path_and_query, tenant_id);
        let uri: hyper::Uri = format!("http://{upstream_addr}{clean_path}")
            .parse()
            .map_err(|e| WrapperError::Proxy(format!("bad upstream uri: {e}")))?;
        parts.uri = uri;
        let host = upstream_addr
            .parse::<hyper::header::HeaderValue>()
            .map_err(|e| WrapperError::Proxy(format!("bad upstream host: {e}")))?;
        parts.headers.insert(hyper::header::HOST, host);
        if let Ok(id) = tenant_id.parse::<hyper::header::HeaderValue>() {
            parts.headers.insert("x-tenant-id", id);
        }

        let body_bytes = body
            .collect()
            .await
            .map_err(|e| WrapperError::Proxy(format!("read request body: {e}")))?
            .to_bytes();
        let upstream_req = hyper::Request::from_parts(parts, Full::new(body_bytes));

        let upstream_res = self
            .client
            .request(upstream_req)
            .await
            .map_err(|e| WrapperError::Proxy(format!("upstream {upstream_addr}: {e}")))?;
        let (res_parts, res_body) = upstream_res.into_parts();
        let bytes: Bytes = res_body
            .collect()
            .await
            .map_err(|e| WrapperError::Proxy(format!("read upstream body: {e}")))?
            .to_bytes();
        Ok(hyper::Response::from_parts(res_parts, Full::new(bytes)))
    }
}

/// Strip a leading `/tenant_<id>` segment (segment-boundary aware) so the
/// upstream zw sees clean app paths. Query strings are preserved.
fn strip_tenant_prefix(path_and_query: &str, tenant_id: &str) -> String {
    if tenant_id == "default" {
        return path_and_query.to_string();
    }
    let seg = if tenant_id.starts_with("tenant_") {
        tenant_id.to_string()
    } else {
        format!("tenant_{tenant_id}")
    };
    let prefixed = format!("/{seg}");
    let rest = match path_and_query.strip_prefix(prefixed.as_str()) {
        Some(r) => r,
        None => return path_and_query.to_string(),
    };
    // Only strip on a segment boundary: end, '/' or '?' (query).
    match rest.chars().next() {
        None => "/".to_string(),
        Some('/') => rest.to_string(),
        Some('?') => format!("/{rest}"),
        Some(_) => path_and_query.to_string(),
    }
}

/// Fast liveness probe of a recorded `IP:port` (800ms budget). True when the
/// VM accepts TCP — enough to trust the address; the forwarded request itself
/// is the real health check.
async fn probe_upstream(upstream_addr: &str) -> bool {
    let fut = tokio::net::TcpStream::connect(upstream_addr);
    tokio::time::timeout(std::time::Duration::from_millis(800), fut)
        .await
        .is_ok_and(|r| r.is_ok())
}
