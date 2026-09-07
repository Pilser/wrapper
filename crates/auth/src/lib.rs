//! `auth` — login, tokens, sessions, permissions, oauth, mfa and audit.

pub mod audit;
pub mod login;
pub mod mfa;
pub mod oauth;
pub mod permissions;
pub mod session;
pub mod tokens;

pub use login::LoginHandler;
pub use session::SessionStore;
pub use tokens::TokenManager;

use telemetry::Telemetry;

/// Facade over the auth subsystem.
#[derive(Clone)]
pub struct AuthService {
    pub telemetry: Telemetry,
}

impl AuthService {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }
}
