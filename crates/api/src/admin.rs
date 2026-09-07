//! Admin handle for plugins (delegates to the `admin` crate).

/// Thin wrapper exposing admin operations to plugins.
#[derive(Clone, Debug)]
pub struct AdminApi {
    pub telemetry: telemetry::Telemetry,
}

impl AdminApi {
    pub fn new(telemetry: telemetry::Telemetry) -> Self {
        Self { telemetry }
    }

    /// Build the underlying [`admin::AdminService`].
    pub fn service(&self) -> admin::AdminService {
        admin::AdminService::new(self.telemetry.clone())
    }

    /// List categories via `admin::categories::list_categories`.
    pub async fn list_categories(
        &self,
        gdb: &db::GlobalDb,
    ) -> shared::Result<Vec<shared::Category>> {
        admin::categories::list_categories(gdb).await
    }
}
