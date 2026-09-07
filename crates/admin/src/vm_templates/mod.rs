//! VM templates (rows in the global `vm_templates` table).

pub mod list;
pub mod map_category;
pub mod register;
pub mod update;
pub mod validate;

pub use list::list_templates;
pub use map_category::map_category_to_template;
pub use register::{register_template, RegisterTemplate};
pub use update::{update_template, UpdateTemplate};
pub use validate::validate_template;
