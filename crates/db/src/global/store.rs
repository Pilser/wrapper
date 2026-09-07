//! The global database handle.

use core::now_rfc3339;
use telemetry::Telemetry;
use turso::Builder;

use crate::migration;

#[derive(Clone)]
pub struct GlobalDb {
    pub db: turso::Database,
    pub path: String,
    pub telemetry: Telemetry,
}

impl GlobalDb {
    pub async fn open(path: impl Into<String>) -> anyhow::Result<Self> {
        let path = path.into();
        let db = Builder::new_local(&path).build().await?;
        let telemetry = Telemetry::noop();
        let conn = db.connect()?;
        migration::global::migrate(&conn).await?;
        Ok(Self { db, path, telemetry })
    }

    /// Attach an existing telemetry handle.
    pub fn with_telemetry(mut self, telemetry: Telemetry) -> Self {
        self.telemetry = telemetry;
        self
    }

    pub fn conn(&self) -> anyhow::Result<turso::Connection> {
        Ok(self.db.connect()?)
    }

    pub fn timestamp(&self) -> String {
        now_rfc3339()
    }
}

/// Facade helpers over the global database.
pub struct GlobalStore {
    pub db: GlobalDb,
}
