//! Generic filter for list queries.

#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    pub tenant_id: Option<String>,
    pub status: Option<String>,
    pub category_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
}
