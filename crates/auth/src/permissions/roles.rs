//! User roles.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    Student,
    Staff,
    Admin,
    ReadOnly,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Student => "student",
            Role::Staff => "staff",
            Role::Admin => "admin",
            Role::ReadOnly => "read_only",
        }
    }
}
