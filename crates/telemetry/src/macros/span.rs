//! `span!` — time a block and emit a `span.completed` event.

/// Wrap a block that returns a `Result` and emit a completed span with its
/// duration and success/failure status.
///
/// ```ignore
/// let res = span!(telemetry.clone(), "db.query.categories", { store.list().await });
/// ```
#[macro_export]
macro_rules! span {
    ($telemetry:expr, $name:expr, $body:block) => {{
        let timer = $crate::event::span_timer($name);
        let result = { $body };
        let status = if result.is_ok() {
            $crate::models::SpanStatus::Success
        } else {
            $crate::models::SpanStatus::Failed
        };
        $telemetry.builder().kind("span.completed").category("performance").span(timer.finish(status)).message("Operation completed").emit();
        result
    }};
}