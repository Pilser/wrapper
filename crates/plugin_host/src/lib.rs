//! `plugin_host` — loads `.so` plugins, watches the plugin folder, dispatches
//! lifecycle hooks and exposes an inter-plugin API registry.

pub mod api;
pub mod hooks;
pub mod lifecycle;
pub mod loader;
pub mod registry;
pub mod sandbox;
pub mod watcher;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use telemetry::Telemetry;

pub use core::{
    HookPoint, Plugin, PluginContext, PluginInfo, PluginType, PluginVersion, Result,
};

use crate::hooks::HookDispatcher;
use crate::loader::{load_library, unload_plugin};
use crate::registry::PluginRegistry;
use crate::watcher::FolderWatcher;

/// The host managing all loaded plugins.
pub struct PluginHost {
    pub folder: PathBuf,
    pub telemetry: Telemetry,
    pub registry: Mutex<PluginRegistry>,
    pub hooks: HookDispatcher,
    pub watcher: Arc<FolderWatcher>,
}

impl PluginHost {
    pub fn new(folder: impl Into<PathBuf>) -> anyhow::Result<Self> {
        Self::with_telemetry(folder, Telemetry::noop())
    }

    pub fn with_telemetry(folder: impl Into<PathBuf>, telemetry: Telemetry) -> anyhow::Result<Self> {
        let folder = folder.into();
        std::fs::create_dir_all(&folder)?;
        Ok(Self {
            folder: folder.clone(),
            telemetry,
            registry: Mutex::new(PluginRegistry::new()),
            hooks: HookDispatcher::new(),
            watcher: Arc::new(FolderWatcher::new(folder.clone())),
        })
    }

    pub fn init(&self) -> anyhow::Result<()> {
        for name in self
            .watcher
            .scan()
            .map_err(|e| anyhow::anyhow!("plugin scan failed: {e}"))?
        {
            let _ = self.load(&name);
        }
        Ok(())
    }

    pub fn load(&self, path: &std::path::Path) -> anyhow::Result<String> {
        let loaded = load_library(path, self.telemetry.clone())?;
        let name = loaded.info.name.clone();
        self.registry.lock().unwrap().store(name.clone(), loaded);
        self.hooks.dispatch(HookPoint::OnPluginLoaded, &serde_json::json!({"name": name}));
        Ok(name)
    }

    pub fn unload(&self, name: &str) -> anyhow::Result<()> {
        let plugin = self.registry.lock().unwrap().remove(name);
        if let Some(p) = plugin {
            unload_plugin(&p, &self.telemetry)?;
        }
        self.hooks.dispatch(HookPoint::OnPluginUnloaded, &serde_json::json!({"name": name}));
        Ok(())
    }

    pub fn reload(&self, name: &str) -> anyhow::Result<()> {
        let path = self.folder.join(format!("{name}.so"));
        self.unload(name)?;
        self.load(&path)?;
        Ok(())
    }

    pub fn list(&self) -> Vec<String> {
        self.registry.lock().unwrap().names()
    }

    pub fn registry(&self) -> std::sync::MutexGuard<'_, PluginRegistry> {
        self.registry.lock().unwrap()
    }
}


