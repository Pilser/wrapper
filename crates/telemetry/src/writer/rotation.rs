//! Daily file rotation and retention pruning.

use std::path::Path;

use crate::config::DiskSettings;

/// Tracks the current writing date and prunes files older than the retention.
pub struct Rotation {
    current: Option<String>,
}

impl Rotation {
    pub fn new() -> Self {
        Self { current: None }
    }

    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Called when the writing date changes; enforces retention.
    pub fn retain(&mut self, settings: &DiskSettings, new_date: &str) -> anyhow::Result<()> {
        self.current = Some(new_date.to_string());
        if settings.retention_days == 0 {
            return Ok(());
        }
        let dir = Path::new(&settings.path);
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            // Only prune files matching the pattern's date slot.
            if !file_name.starts_with("telemetry_") || !file_name.ends_with(".json") {
                continue;
            }
            let date_part = file_name
                .trim_start_matches("telemetry_")
                .trim_end_matches(".json");
            if date_part.len() == 10 && is_older_than(date_part, settings.retention_days) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

impl Default for Rotation {
    fn default() -> Self {
        Self::new()
    }
}

/// Best-effort check that `date` is older than `days`.
fn is_older_than(date: &str, days: u32) -> bool {
    let Ok(parsed) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
        return false;
    };
    let today = chrono::Utc::now().date_naive();
    let Some(limit) = today.checked_sub_days(chrono::Days::new(days as u64)) else {
        return false;
    };
    parsed < limit
}