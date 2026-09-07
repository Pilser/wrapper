//! VM template registry and builder.

pub mod builder;
pub mod category;
pub mod registry;
pub mod validate;
pub mod version;

pub use builder::VmTemplateBuilder;
pub use registry::TemplateRegistry;
