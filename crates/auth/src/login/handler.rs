//! Login request handler.

use telemetry::{Level, Telemetry};

use super::validate::validate_credentials;
use crate::session::SessionStore;
use crate::tokens::TokenManager;

/// Orchestrates a login attempt.
#[derive(Clone)]
pub struct LoginHandler {
    pub sessions: SessionStore,
    pub tokens: TokenManager,
    pub telemetry: Telemetry,
}

impl LoginHandler {
    pub fn new(sessions: SessionStore, tokens: TokenManager, telemetry: Telemetry) -> Self {
        Self { sessions, tokens, telemetry }
    }

    /// Validate credentials and mint a session + token.
    pub async fn login(&self, username: &str, password: &str) -> anyhow::Result<Option<String>> {
        if !validate_credentials(username, password) {
            self.telemetry.builder().level(Level::Warn).kind("auth.login.failed")
                .category("auth").message(format!("login failed for {username}")).emit();
            return Ok(None);
        }
        let token = self.tokens.mint(username)?;
        self.sessions.create(username, &token)?;
        self.telemetry.builder().level(Level::Info).kind("auth.login.success")
            .category("auth").message(format!("login success for {username}")).emit();
        Ok(Some(token))
    }

    pub async fn logout(&self, token: &str) -> anyhow::Result<()> {
        self.tokens.revoke(token)?;
        self.sessions.destroy(token)?;
        self.telemetry.builder().level(Level::Info).kind("auth.logout.success")
            .category("auth").message("logout").emit();
        Ok(())
    }
}
