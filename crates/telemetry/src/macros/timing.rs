//! `timing!` — measure wall-clock time of an expression.

/// Evaluate an expression and return `(result, elapsed)`.
///
/// ```ignore
/// let (rows, elapsed) = timing!(conn.query(...).await);
/// ```
#[macro_export]
macro_rules! timing {
    ($expr:expr) => {{
        let __start = std::time::Instant::now();
        let __result = $expr;
        (__result, __start.elapsed())
    }};
}