//! A tiny SQL builder producing `(sql, params)` pairs.

/// Accumulates a parameterized SQL statement.
#[derive(Debug, Default)]
pub struct QueryBuilder {
    sql: String,
    params: Vec<turso::Value>,
}

impl QueryBuilder {
    pub fn new(stmt: impl Into<String>) -> Self {
        Self { sql: stmt.into(), params: Vec::new() }
    }

    pub fn append(mut self, clause: &str) -> Self {
        self.sql.push(' ');
        self.sql.push_str(clause);
        self
    }

    pub fn bind(mut self, value: impl Into<turso::Value>) -> Self {
        self.params.push(value.into());
        self
    }

    pub fn bind_text(mut self, value: impl Into<String>) -> Self {
        self.params.push(turso::Value::Text(value.into()));
        self
    }

    pub fn bind_int(mut self, value: i64) -> Self {
        self.params.push(turso::Value::Integer(value));
        self
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn params(&self) -> Vec<turso::Value> {
        self.params.clone()
    }

    /// Run the built query with the collected params.
    pub async fn query(self, conn: &turso::Connection) -> anyhow::Result<turso::Rows> {
        Ok(conn.query(self.sql, self.params).await?)
    }
}
