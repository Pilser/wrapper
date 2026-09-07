//! Auth handle for plugins (delegates to the `auth` crate).

/// Thin wrapper exposing auth to plugins.
#[derive(Clone, Debug)]
pub struct AuthApi {
    pub telemetry: telemetry::Telemetry,
}

impl AuthApi {
    pub fn new(telemetry: telemetry::Telemetry) -> Self {
        Self { telemetry }
    }

    /// Build the underlying [`auth::AuthService`].
    pub fn service(&self) -> auth::AuthService {
        auth::AuthService::new(self.telemetry.clone())
    }
}
