//! Auth middleware stub: requires an `authorization` header.

use auth::AuthService;
use shared::{Error as WrapperError, Result};
use telemetry::{Level, Telemetry};

/// Rejects requests without an authorization header.
#[derive(Clone)]
pub struct AuthMiddleware {
    telemetry: Telemetry,
    auth: AuthService,
}

impl std::fmt::Debug for AuthMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthMiddleware").finish()
    }
}

impl AuthMiddleware {
    pub fn new(telemetry: Telemetry) -> Self {
        Self {
            auth: AuthService::new(telemetry.clone()),
            telemetry,
        }
    }

    pub fn auth_service(&self) -> &AuthService {
        &self.auth
    }

    pub async fn check(&self, req: &hyper::Request<hyper::body::Incoming>) -> Result<()> {
        let present = req
            .headers()
            .contains_key(hyper::header::AUTHORIZATION);
        if present {
            self.telemetry
                .builder()
                .level(Level::Info)
                .kind("proxy.request.received")
                .category("proxy")
                .source("proxy::middleware::auth")
                .message("auth middleware passed")
                .emit();
            return Ok(());
        }
        self.telemetry
            .builder()
            .level(Level::Error)
            .kind("auth.permission.denied")
            .category("proxy")
            .source("proxy::middleware::auth")
            .message("missing authorization header")
            .emit();
        Err(WrapperError::Unauthorized(
            "missing authorization header".to_string(),
        ))
    }
}
