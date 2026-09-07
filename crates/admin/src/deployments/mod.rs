//! Category-based deployment packages: configs, plugins, files and commands.

pub mod commands;
pub mod config;
pub mod configs;
pub mod files;
pub mod plugins;
pub mod preview;

pub use commands::queue_command;
pub use config::{configure_deployment, ConfigureDeployment};
pub use configs::{push_config, remove_config};
pub use files::{push_file, remove_file};
pub use plugins::{add_plugin, remove_plugin, DeploymentPlugin};
pub use preview::{preview_deployment, DeploymentPreview};
