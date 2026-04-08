pub mod auth;
pub mod chain_connect;
pub mod channel;
pub mod exit_proxy;
pub mod forward;
pub mod host_key;
pub mod proxy;
pub mod proxy_command;
pub mod reverse_forward;
pub mod session;
pub mod socks5;

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

    #[error("proxy connection failed: {0}")]
    ProxyFailed(String),
}
