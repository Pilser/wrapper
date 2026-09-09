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

        // WebSocket upgrades bypass transforms+sanitize (which strips the
        // Upgrade/Connection headers) and are tunneled byte-for-byte to zw:
        // streams, file downloads and tagged images must flow without
        // blocking. Same wake/probe/wait semantics as plain HTTP below.
        if crate::websocket::is_upgrade_request(&req) {
            let tenant_id = self.router.route_with_telemetry(&req).await?;
            let (upstream_addr, woken) = self.resolve_upstream(&tenant_id).await?;
            if woken {
                self.warn_if_slow(&upstream_addr, &tenant_id).await;
            }
            return self.tunnel_upgrade(upstream_addr, tenant_id, req).await;
        }

        self.request_transform.apply(&mut req)?;
        crate::transform::sanitize(req.headers_mut());

        let tenant_id = self.router.route_with_telemetry(&req).await?;

        // Resolve the tenant's VM address. If the tenant has no running VM
        // (lookup returns the default target) wake one up on demand. A
        // recorded address is PROBED first: a dead VM (crashed, reaped)
        // must re-wake, never black-hole into a stale IP. A freshly woken
        // VM needs ~40s before zw answers, so the wake path WAITS (retry
        // probe) instead of failing the request that triggered the boot.
        let (upstream_addr, woken) = self.resolve_upstream(&tenant_id).await?;
        if woken {
            self.warn_if_slow(&upstream_addr, &tenant_id).await;
        }

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

    /// Resolve tenant -> upstream `IP:port`, waking when missing or dead.
    /// Returns the address plus whether a (re)wake just happened (callers
    /// that need a live app behind the address should wait on zw next).
    async fn resolve_upstream(&self, tenant_id: &str) -> Result<(String, bool)> {
        let upstream_addr = self.lookup.lookup(tenant_id).await?;
        if upstream_addr == self.lookup.default_target() {
            return Ok((self.wake(tenant_id).await?, true));
        }
        if probe_upstream(&upstream_addr).await {
            return Ok((upstream_addr, false));
        }
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
        Ok((self.wake(tenant_id).await?, true))
    }

    /// Warn when a freshly woken VM still doesn't answer (boot slower than
    /// the wait budget). Callers forward anyway and surface the real error.
    async fn warn_if_slow(&self, upstream_addr: &str, tenant_id: &str) {
        if !wait_for_zw(upstream_addr).await {
            self.telemetry
                .builder()
                .level(telemetry::Level::Warn)
                .kind("proxy.vm.boot_slow")
                .category("proxy")
                .source("proxy::http::handler")
                .message(format!(
                    "VM {upstream_addr} for {tenant_id} still dark after wake; forwarding anyway"
                ))
                .emit();
        }
    }

    /// Tunnel a WebSocket upgrade to zw byte-for-byte (streams, file/image
    /// downloads). The 101 handshake headers come straight from zw; after
    /// that both directions are piped until either side closes. Message
    /// framing is zw's business — the proxy never decodes WS frames.
    async fn tunnel_upgrade(
        &self,
        upstream_addr: String,
        tenant_id: String,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Response<Full<Bytes>>> {
        if let Some(activity) = &self.activity {
            activity.touch(&tenant_id);
        }
        let (mut parts, body) = req.into_parts();
        let path_and_query = parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        let clean_path = strip_tenant_prefix(&path_and_query, &tenant_id);
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
        // Upgrade handshakes are GETs without bodies; the server side has
        // already parsed the headers, so the body stream is untouched.
        // Keep the handshake headers (key/version/upgrade/connection),
        // rewrite Host, stamp the tenant.
        let client_req = hyper::Request::from_parts(parts, body);
        let mut up_builder = hyper::Request::builder()
            .method(hyper::Method::GET)
            .uri(format!("http://{upstream_addr}{clean_path}"));
        for (name, value) in client_req.headers().iter() {
            if *name != hyper::header::HOST {
                up_builder = up_builder.header(name.clone(), value.clone());
            }
        }
        if let Ok(h) = upstream_addr.parse::<hyper::header::HeaderValue>() {
            up_builder = up_builder.header(hyper::header::HOST, h);
        }
        if let Ok(id) = tenant_id.parse::<hyper::header::HeaderValue>() {
            up_builder = up_builder.header("x-tenant-id", id);
        }
        let up_req = up_builder
            .body(Full::new(Bytes::new()))
            .map_err(|e| WrapperError::Proxy(format!("bad upstream ws request: {e}")))?;

        let up_res = self
            .client
            .request(up_req)
            .await
            .map_err(|e| WrapperError::Proxy(format!("upstream ws {upstream_addr}: {e}")))?;
        if up_res.status() != hyper::StatusCode::SWITCHING_PROTOCOLS {
            // zw refused the upgrade (e.g. 404/400): pass the response back.
            let (res_parts, res_body) = up_res.into_parts();
            let bytes: Bytes = res_body
                .collect()
                .await
                .map_err(|e| WrapperError::Proxy(format!("read upstream ws body: {e}")))?
                .to_bytes();
            return Ok(hyper::Response::from_parts(res_parts, Full::new(bytes)));
        }
        // 101: pipe both upgraded streams until either side closes.
        let mut builder = hyper::Response::builder().status(hyper::StatusCode::SWITCHING_PROTOCOLS);
        for (name, value) in up_res.headers().iter() {
            builder = builder.header(name.clone(), value.clone());
        }
        let res = builder
            .body(Full::new(Bytes::new()))
            .map_err(|e| WrapperError::Proxy(format!("ws 101 response: {e}")))?;
        let telemetry = self.telemetry.clone();
        tokio::spawn(async move {
            match (
                hyper::upgrade::on(client_req).await,
                hyper::upgrade::on(up_res).await,
            ) {
                (Ok(client_io), Ok(upstream_io)) => {
                    let mut client_io = hyper_util::rt::TokioIo::new(client_io);
                    let mut upstream_io = hyper_util::rt::TokioIo::new(upstream_io);
                    let _ = tokio::io::copy_bidirectional(&mut client_io, &mut upstream_io).await;
                }
                (c, u) => {
                    telemetry
                        .builder()
                        .level(telemetry::Level::Warn)
                        .kind("proxy.websocket.failed")
                        .category("proxy")
                        .source("proxy::http::handler")
                        .message(format!(
                            "ws tunnel upgrade failed client_ok={} upstream_ok={}",
                            c.is_ok(),
                            u.is_ok()
                        ))
                        .emit();
                }
            }
        });
        self.telemetry
            .builder()
            .level(Level::Info)
            .kind("proxy.websocket.tunneled")
            .category("proxy")
            .source("proxy::http::handler")
            .message(format!(
                "ws tunneled {clean_path} tenant={tenant_id} to {upstream_addr}"
            ))
            .emit();
        Ok(res)
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
        let bytes = rewrite_base_href(
            bytes,
            res_parts.headers.get(hyper::header::CONTENT_TYPE),
            tenant_id,
        );
        // NOTE: no tenant cookie is planted (a zw_tenant fallback existed
        // and was removed): third-party contexts reject it with console
        // noise, and the base-tag rewrite covers prefixed WS/asset URLs
        // deterministically. Session auth still uses Bearer tokens.
        Ok(hyper::Response::from_parts(res_parts, Full::new(bytes)))
    }
}

/// Rewrite `<base href="/">` in HTML bodies to the tenant-prefixed base so
/// the guest app builds routable URLs itself (zw's JS reads the base tag:
/// `/tenant_<id>/ws` instead of root `/ws`, relative assets stay under the
/// prefix). Without this, root-absolute WS handshakes carry no tenant
/// signal at all — browsers send no Referer on WS, Origin has no path, and
/// third-party cookies get rejected — and the handshake 500s. Standard
/// reverse-proxy base-tag rewriting; capped at 4 MB bodies.
fn rewrite_base_href(
    body: Bytes,
    content_type: Option<&hyper::header::HeaderValue>,
    tenant_id: &str,
) -> Bytes {
    if tenant_id == "default" || body.len() > 4 * 1024 * 1024 {
        return body;
    }
    let is_html = content_type
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/html"));
    if !is_html {
        return body;
    }
    let replacement = format!("<base href=\"/tenant_{tenant_id}/\">");
    let replacement_sq = format!("<base href='/tenant_{tenant_id}/'>");
    let mut text = String::from_utf8_lossy(&body).into_owned();
    let mut count = 0;
    while let Some(pos) = text.find("<base href=\"/\">") {
        text.replace_range(pos..pos + 15, &replacement);
        count += 1;
        if count > 4 {
            break;
        }
    }
    while let Some(pos) = text.find("<base href='/'>") {
        text.replace_range(pos..pos + 15, &replacement_sq);
        count += 1;
        if count > 4 {
            break;
        }
    }
    if count == 0 {
        return body;
    }
    Bytes::from(text)
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

/// Wait for a freshly woken VM's zw to accept TCP (guest boot ~40s).
/// Polls every 2s up to ~50s; false on timeout (caller forwards anyway and
/// surfaces the real upstream error).
async fn wait_for_zw(upstream_addr: &str) -> bool {
    for _ in 0..25 {
        if probe_upstream(upstream_addr).await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    false
}
