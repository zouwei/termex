pub mod manifest;
pub mod registry;

/// Plugin system error types.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("plugin not found: {0}")]
    NotFound(String),

    #[error("plugin already installed: {0}")]
    AlreadyInstalled(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
}
