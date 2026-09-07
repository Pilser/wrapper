use shared::{Error, Result};

/// True when `now_ms - last_seen_ms` exceeds the timeout.
pub fn is_timed_out(last_seen_ms: i64, now_ms: i64, timeout_secs: u64) -> bool {
    now_ms.saturating_sub(last_seen_ms) > timeout_secs as i64 * 1000
}

pub fn check_timeout(last_seen_ms: i64, now_ms: i64, timeout_secs: u64) -> Result<()> {
    if is_timed_out(last_seen_ms, now_ms, timeout_secs) {
        return Err(Error::Timeout("supervisor heartbeat missed".to_string()));
    }
    Ok(())
}
