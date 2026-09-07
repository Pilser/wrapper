//! The file writer: buffers events and appends them to a per-day log file.

use std::path::PathBuf;

use chrono::Datelike;

use super::{buffer::FloatBuffer, rotation::Rotation};
use crate::config::DiskSettings;
use crate::models::Event;

/// Writes events to `<path>/telemetry_<date>.json` (JSON-lines format).
///
/// Events are buffered and flushed when the buffer fills or the day rolls over.
pub struct FileWriter {
    settings: DiskSettings,
    buffer: FloatBuffer,
    rotation: Rotation,
    current_date: Option<String>,
}

impl FileWriter {
    pub fn new(settings: DiskSettings) -> Self {
        Self {
            settings,
            buffer: FloatBuffer::new(),
            rotation: Rotation::new(),
            current_date: None,
        }
    }

    /// Buffer an event; flush early if the buffer is full or the date changed.
    pub async fn write(&mut self, ev: &Event) -> anyhow::Result<()> {
        let date = timestamp_date(&ev.timestamp);
        if self.current_date.as_deref() != Some(date.as_str()) {
            // Date rolled over (or first event): close the previous file.
            self.flush().await?;
            self.rotation.retain(&self.settings, date.as_str())?;
            self.current_date = Some(date);
        }
        self.buffer.push(ev.clone());
        if self.buffer.len() >= self.settings.buffer_size {
            self.flush().await?;
        }
        Ok(())
    }

    /// Flush buffered events to the current day's file.
    pub async fn flush(&mut self) -> anyhow::Result<()> {
        let date = self.current_date.clone().unwrap_or_else(|| today());
        let events = self.buffer.drain();
        if events.is_empty() {
            return Ok(());
        }
        let path = self.file_path(&date);
        let dir = path.parent().unwrap_or_else(|| std::path::Path::new("/"));
        std::fs::create_dir_all(dir)?;
        let mut appended = Vec::new();
        for ev in &events {
            let line = serde_json::to_string(ev)?;
            appended.push(line);
        }
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        for line in &appended {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }

    fn file_path(&self, date: &str) -> PathBuf {
        let name = self.settings.filename_pattern.replace("{date}", date);
        PathBuf::from(&self.settings.path).join(name)
    }
}

/// Extract the `YYYY-MM-DD` portion of an RFC3339 timestamp.
pub fn timestamp_date(ts: &str) -> String {
    ts.chars().take(10).collect()
}

/// Current date string.
pub fn today() -> String {
    let now = chrono::Utc::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
}