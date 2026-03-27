pub mod session;

/// SFTP error types.
#[derive(Debug, thiserror::Error)]
pub enum SftpError {
    #[error("SFTP session not found: {0}")]
    SessionNotFound(String),

    #[error("SSH session not found: {0}")]
    SshSessionNotFound(String),

    #[error("SFTP error: {0}")]
    Sftp(String),

    #[error("channel error: {0}")]
    ChannelError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<russh_sftp::client::error::Error> for SftpError {
    fn from(e: russh_sftp::client::error::Error) -> Self {
        Self::Sftp(e.to_string())
    }
}

impl From<russh::Error> for SftpError {
    fn from(e: russh::Error) -> Self {
        Self::ChannelError(e.to_string())
    }
}
