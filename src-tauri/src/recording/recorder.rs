use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::asciicast::{AsciicastEvent, AsciicastFile, AsciicastHeader};
use super::RecordingError;

/// An active recording session.
struct ActiveRecording {
    header: AsciicastHeader,
    events: Vec<AsciicastEvent>,
    start_time: std::time::Instant,
    file_path: PathBuf,
}

/// Manages recording sessions.
pub struct RecorderRegistry {
    recordings: Arc<RwLock<HashMap<String, ActiveRecording>>>,
}

impl RecorderRegistry {
    /// Creates a new registry.
    pub fn new() -> Self {
        Self {
            recordings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Starts recording a session.
    pub async fn start(
        &self,
        session_id: &str,
        width: u32,
        height: u32,
        title: Option<String>,
    ) -> Result<PathBuf, RecordingError> {
        let dir = recordings_dir()?;
        std::fs::create_dir_all(&dir)?;

        let filename = format!(
            "{}-{}.cast",
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            &session_id[..8],
        );
        let file_path = dir.join(&filename);

        let recording = ActiveRecording {
            header: AsciicastHeader::new(width, height, title),
            events: Vec::new(),
            start_time: std::time::Instant::now(),
            file_path: file_path.clone(),
        };

        self.recordings
            .write()
            .await
            .insert(session_id.to_string(), recording);

        Ok(file_path)
    }

    /// Records an output event.
    pub async fn record_output(&self, session_id: &str, data: &str) {
        let mut recordings = self.recordings.write().await;
        if let Some(rec) = recordings.get_mut(session_id) {
            let elapsed = rec.start_time.elapsed().as_secs_f64();
            rec.events.push(AsciicastEvent::output(elapsed, data));
        }
    }

    /// Records an input event.
    pub async fn record_input(&self, session_id: &str, data: &str) {
        let mut recordings = self.recordings.write().await;
        if let Some(rec) = recordings.get_mut(session_id) {
            let elapsed = rec.start_time.elapsed().as_secs_f64();
            rec.events.push(AsciicastEvent::input(elapsed, data));
        }
    }

    /// Stops recording and writes the file.
    pub async fn stop(&self, session_id: &str) -> Result<PathBuf, RecordingError> {
        let recording = self
            .recordings
            .write()
            .await
            .remove(session_id)
            .ok_or_else(|| RecordingError::NotFound(session_id.to_string()))?;

        let file = AsciicastFile {
            header: recording.header,
            events: recording.events,
        };

        let content = file.serialize()?;
        std::fs::write(&recording.file_path, content)?;

        Ok(recording.file_path)
    }

    /// Checks if a session is being recorded.
    pub async fn is_recording(&self, session_id: &str) -> bool {
        self.recordings.read().await.contains_key(session_id)
    }
}

/// Lists all recorded files.
pub fn list_recordings() -> Result<Vec<RecordingInfo>, RecordingError> {
    let dir = recordings_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut recordings = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "cast") {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size = entry.metadata()?.len();
            recordings.push(RecordingInfo {
                filename,
                path: path.to_string_lossy().to_string(),
                size,
            });
        }
    }

    recordings.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(recordings)
}

/// Information about a recorded file.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub filename: String,
    pub path: String,
    pub size: u64,
}

/// Returns the directory where recordings are stored.
fn recordings_dir() -> Result<PathBuf, RecordingError> {
    let data_dir = dirs::data_dir().ok_or_else(|| {
        RecordingError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no data directory",
        ))
    })?;
    Ok(data_dir.join("termex").join("recordings"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_recorder_lifecycle() {
        let registry = RecorderRegistry::new();

        assert!(!registry.is_recording("test-session").await);

        let _path = registry
            .start("test-session", 80, 24, Some("test".into()))
            .await
            .unwrap();

        assert!(registry.is_recording("test-session").await);

        registry.record_output("test-session", "$ ").await;
        registry.record_input("test-session", "ls\r").await;
        registry.record_output("test-session", "file.txt\r\n").await;

        // Stop and verify
        // Note: this writes to disk, but in test we can verify the path exists
        let result_path = registry.stop("test-session").await.unwrap();
        assert!(result_path.exists());

        // Verify content
        let content = std::fs::read_to_string(&result_path).unwrap();
        let file = super::super::asciicast::AsciicastFile::parse(&content).unwrap();
        assert_eq!(file.events.len(), 3);
        assert_eq!(file.header.width, 80);

        // Cleanup
        let _ = std::fs::remove_file(result_path);
    }
}
