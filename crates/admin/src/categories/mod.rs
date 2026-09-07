//! Tenant categories (rows in the global `categories` table).

pub mod assign;
pub mod create;
pub mod delete;
pub mod list;
pub mod update;

pub use assign::{assign_category, CategoryAssignment};
pub use create::{create_category, CreateCategory};
pub use delete::delete_category;
pub use list::list_categories;
pub use update::{update_category, UpdateCategory};
