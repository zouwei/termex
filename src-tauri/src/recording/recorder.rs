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
    recording_id: String,
    server_id: String,
    server_name: String,
    auto_recorded: bool,
    accumulated_bytes: usize,
    max_bytes: usize,
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
    ///
    /// Files are archived by server_id and date:
    /// `recordings/{server_id}/{YYYY-MM-DD}/{recording_id}.cast`
    pub async fn start(
        &self,
        session_id: &str,
        server_id: &str,
        server_name: &str,
        width: u32,
        height: u32,
        title: Option<String>,
        auto_recorded: bool,
        max_recording_mb: u32,
    ) -> Result<(String, PathBuf), RecordingError> {
        let recording_id = uuid::Uuid::new_v4().to_string();

        let now = time::OffsetDateTime::now_utc();
        let date_str = format!(
            "{:04}-{:02}-{:02}",
            now.year(),
            now.month() as u8,
            now.day()
        );
        let dir = recordings_dir()?.join(server_id).join(&date_str);
        std::fs::create_dir_all(&dir)?;

        let file_path = dir.join(format!("{}.cast", &recording_id));

        let recording = ActiveRecording {
            header: AsciicastHeader::new(width, height, title),
            events: Vec::new(),
            start_time: std::time::Instant::now(),
            file_path: file_path.clone(),
            recording_id: recording_id.clone(),
            server_id: server_id.to_string(),
            server_name: server_name.to_string(),
            auto_recorded,
            accumulated_bytes: 0,
            max_bytes: max_recording_mb as usize * 1024 * 1024,
        };

        self.recordings
            .write()
            .await
            .insert(session_id.to_string(), recording);

        Ok((recording_id, file_path))
    }

    /// Records an output event. Returns false if size limit is exceeded.
    pub async fn record_output(&self, session_id: &str, data: &str) -> bool {
        let mut recordings = self.recordings.write().await;
        if let Some(rec) = recordings.get_mut(session_id) {
            rec.accumulated_bytes += data.len();
            if rec.max_bytes > 0 && rec.accumulated_bytes > rec.max_bytes {
                return false;
            }
            let elapsed = rec.start_time.elapsed().as_secs_f64();
            rec.events.push(AsciicastEvent::output(elapsed, data));
            return true;
        }
        true // not recording, no limit issue
    }

    /// Records an input event.
    pub async fn record_input(&self, session_id: &str, data: &str) {
        let mut recordings = self.recordings.write().await;
        if let Some(rec) = recordings.get_mut(session_id) {
            let elapsed = rec.start_time.elapsed().as_secs_f64();
            rec.events.push(AsciicastEvent::input(elapsed, data));
        }
    }

    /// Stops recording and writes the file. Returns (recording_id, file_path).
    pub async fn stop(&self, session_id: &str) -> Result<(String, PathBuf), RecordingError> {
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

        Ok((recording.recording_id, recording.file_path))
    }

    /// Checks if a session is being recorded.
    pub async fn is_recording(&self, session_id: &str) -> bool {
        self.recordings.read().await.contains_key(session_id)
    }

    /// Gets the recording_id for an active session.
    pub async fn get_recording_id(&self, session_id: &str) -> Option<String> {
        self.recordings
            .read()
            .await
            .get(session_id)
            .map(|r| r.recording_id.clone())
    }
}

/// Lists all recorded files (legacy flat scan, kept for backward compat).
pub fn list_recordings() -> Result<Vec<RecordingInfo>, RecordingError> {
    let dir = recordings_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut recordings = Vec::new();
    scan_dir_recursive(&dir, &mut recordings)?;
    recordings.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(recordings)
}

fn scan_dir_recursive(
    dir: &std::path::Path,
    out: &mut Vec<RecordingInfo>,
) -> Result<(), RecordingError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir_recursive(&path, out)?;
        } else if path.extension().map_or(false, |e| e == "cast") {
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let size = entry.metadata()?.len();
            out.push(RecordingInfo {
                filename,
                path: path.to_string_lossy().to_string(),
                size,
            });
        }
    }
    Ok(())
}

/// Information about a recorded file.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub filename: String,
    pub path: String,
    pub size: u64,
}

/// Returns the directory where recordings are stored (portable-aware).
fn recordings_dir() -> Result<PathBuf, RecordingError> {
    Ok(crate::paths::recordings_dir())
}
