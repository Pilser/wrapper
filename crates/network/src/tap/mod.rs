//! Tap device lifecycle.

pub mod configure;
pub mod create;
pub mod destroy;
pub mod list;

pub use configure::configure_tap;
pub use create::{create_tap, TapCreateOptions};
pub use destroy::destroy_tap;
pub use list::list_taps;

use serde::{Deserialize, Serialize};

/// A created tap device bound to a bridge/network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapDevice {
    pub name: String,
    pub ip: String,
    pub owner: Option<String>,
}
