pub mod asciicast;
pub mod recorder;

/// Recording error types.
#[derive(Debug, thiserror::Error)]
pub enum RecordingError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("recording not found: {0}")]
    NotFound(String),

    #[error("not recording")]
    NotRecording,
}
