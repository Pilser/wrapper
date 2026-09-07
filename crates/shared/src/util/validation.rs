//! Input validation helpers for ids, usernames and addresses.

/// Validate a UUID v4 string.
pub fn is_valid_uuid(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok()
}

/// Validate a generic tenant/entity id `[a-zA-Z0-9_-]+`.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Validate a username.
pub fn is_valid_username(username: &str) -> bool {
    let len = username.len();
    (3..=64).contains(&len)
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

/// Validate a category name.
pub fn is_valid_category_name(name: &str) -> bool {
    let len = name.len();
    (1..=64).contains(&len)
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ' ')
}