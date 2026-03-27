pub mod auth;
pub mod channel;
pub mod forward;
pub mod session;

/// SSH error types.
#[derive(Debug, thiserror::Error)]
pub enum SshError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("channel error: {0}")]
    ChannelError(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("session already disconnected")]
    AlreadyDisconnected,

    #[error("server not found: {0}")]
    ServerNotFound(String),

    #[error("russh error: {0}")]
    Russh(#[from] russh::Error),

    #[error("key error: {0}")]
    KeyError(#[from] russh_keys::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto error: {0}")]
    Crypto(String),
}
