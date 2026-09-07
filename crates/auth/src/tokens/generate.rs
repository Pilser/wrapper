//! Token minting.

use uuid::Uuid;

/// Mint a new opaque token.
pub fn mint_token(subject: &str) -> String {
    format!("tk_{}_{}", subject, Uuid::new_v4().simple())
}

pub fn generate_token() -> String {
    Uuid::new_v4().simple().to_string()
}
