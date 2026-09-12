//! JSON helpers for `/api` handlers: body parsing, responses, errors.
//!
//! Every response is JSON (`{...}` or `{error, detail?}`) so frontends parse
//! one shape. Errors never leak internals (no backtraces, no SQL).

use bytes::Bytes;
use http_body_util::{BodyExt, Full};

use super::ApiResponse;

const BODY_LIMIT: usize = 1024 * 1024;

pub fn json(status: hyper::StatusCode, value: serde_json::Value) -> ApiResponse {
    let body = value.to_string();
    hyper::Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

pub fn ok(value: serde_json::Value) -> ApiResponse {
    json(hyper::StatusCode::OK, value)
}

pub fn created(value: serde_json::Value) -> ApiResponse {
    json(hyper::StatusCode::CREATED, value)
}

pub fn accepted(value: serde_json::Value) -> ApiResponse {
    json(hyper::StatusCode::ACCEPTED, value)
}

pub fn bad_request(msg: impl Into<String>) -> ApiResponse {
    json(
        hyper::StatusCode::BAD_REQUEST,
        serde_json::json!({"error": msg.into()}),
    )
}

pub fn unauthorized() -> ApiResponse {
    json(
        hyper::StatusCode::UNAUTHORIZED,
        serde_json::json!({"error": "invalid or expired session"}),
    )
}

pub fn forbidden(msg: impl Into<String>) -> ApiResponse {
    json(
        hyper::StatusCode::FORBIDDEN,
        serde_json::json!({"error": msg.into()}),
    )
}

pub fn not_found(msg: impl Into<String>) -> ApiResponse {
    json(
        hyper::StatusCode::NOT_FOUND,
        serde_json::json!({"error": msg.into()}),
    )
}

pub fn conflict(msg: impl Into<String>) -> ApiResponse {
    json(
        hyper::StatusCode::CONFLICT,
        serde_json::json!({"error": msg.into()}),
    )
}

pub fn unavailable(msg: impl Into<String>) -> ApiResponse {
    json(
        hyper::StatusCode::SERVICE_UNAVAILABLE,
        serde_json::json!({"error": msg.into()}),
    )
}

pub fn internal() -> ApiResponse {
    json(
        hyper::StatusCode::INTERNAL_SERVER_ERROR,
        serde_json::json!({"error": "internal error"}),
    )
}

/// CORS preflight answer for browser frontends (Cloudflare Pages).
/// Token auth uses `Authorization` (Bearer), never cookies, so a wildcard
/// origin is safe here; lock down to the Pages domain in production.
pub fn cors_preflight() -> ApiResponse {
    with_cors(
        hyper::Response::builder()
            .status(hyper::StatusCode::NO_CONTENT)
            .body(Full::new(Bytes::new()))
            .unwrap(),
    )
}

/// Tag a response with the CORS headers browsers require.
pub fn with_cors(mut res: ApiResponse) -> ApiResponse {
    res.headers_mut().insert(
        hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        hyper::header::HeaderValue::from_static("*"),
    );
    res.headers_mut().insert(
        hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
        hyper::header::HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
    );
    res.headers_mut().insert(
        hyper::header::ACCESS_CONTROL_ALLOW_HEADERS,
        hyper::header::HeaderValue::from_static("Authorization, Content-Type"),
    );
    res.headers_mut().insert(
        hyper::header::ACCESS_CONTROL_MAX_AGE,
        hyper::header::HeaderValue::from_static("86400"),
    );
    res
}

/// Read a JSON body (1MB cap). Err responses are already shaped.
pub async fn read_json(
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<serde_json::Value, ApiResponse> {
    let bytes = req
        .into_body()
        .collect()
        .await
        .map_err(|_| bad_request("unreadable body"))?
        .to_bytes();
    if bytes.len() > BODY_LIMIT {
        return Err(bad_request("body too large"));
    }
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|_| bad_request("invalid JSON body"))
}

/// `Authorization: Bearer <token>` (case-insensitive scheme).
pub fn bearer(req: &hyper::Request<hyper::body::Incoming>) -> Option<String> {
    req.headers()
        .get(hyper::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").or_else(|| s.strip_prefix("bearer ")))
        .map(str::to_string)
}

/// Last path segment (`/api/admin/tenants/{id}` -> `{id}`).
pub fn last_seg(req: &hyper::Request<hyper::body::Incoming>) -> String {
    req.uri()
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .next_back()
        .unwrap_or_default()
        .to_string()
}

/// Second-to-last segment (`.../tenants/{id}/suspend` -> `{id}`).
pub fn second_last_seg(req: &hyper::Request<hyper::body::Incoming>) -> String {
    nth_from_last_seg(req, 2)
}

/// Nth segment from the end (1 = last): `.../tenants/{id}/disk/resize`
/// with n=3 yields `{id}`.
pub fn nth_from_last_seg(req: &hyper::Request<hyper::body::Incoming>, n: usize) -> String {
    let segs: Vec<&str> = req
        .uri()
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    segs.get(segs.len().saturating_sub(n)).unwrap_or(&"").to_string()
}
