use serde_json::Value;

/// Extract the category a deployment targets.
/// Reads the shared-manifest `category` field, with legacy `category_id`
/// fallback.
pub fn category_id_from(payload: &Value) -> Option<String> {
    payload
        .get("category")
        .and_then(|v| v.as_str())
        .or_else(|| {
            payload
                .get("category_id")
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string())
}

pub fn describe_package(payload: &Value) -> Value {
    serde_json::json!({
        "category": category_id_from(payload),
        "tenant_id": payload.get("tenant_id"),
    })
}
