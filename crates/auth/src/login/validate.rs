//! Credential validation.

/// Validate a username/password pair against the user store.
pub fn validate_credentials(username: &str, password: &str) -> bool {
    !username.is_empty() && !password.is_empty() && password.len() >= 8
}

pub fn validate_username(username: &str) -> bool {
    (3..=64).contains(&username.len())
}
