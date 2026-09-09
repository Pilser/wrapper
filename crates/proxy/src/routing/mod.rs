//! Request routing: tenant extraction, VM lookup and fallback.

pub mod activity;
pub mod fallback;
pub mod router;
pub mod tenant;
pub mod vm_lookup;

pub use activity::ActivityMap;

pub use fallback::FallbackHandler;
pub use router::Router;
pub use tenant::{resolve, tenant_id_of, TenantRoute};
pub use vm_lookup::VmLookup;
