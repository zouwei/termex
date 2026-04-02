use std::sync::Arc;

use russh::client;

use super::auth::ClientHandler;
use super::channel::{ChannelCommand, ChannelHandle, spawn_channel_task};
use super::proxy::{self, ProxyConfig};
use super::SshError;
use crate::sftp::session::SftpHandle;

/// Represents an active SSH session with its shell channel.
///
/// All operations (write, resize) are non-blocking message sends,
/// avoiding async issues with lock guards.
pub struct SshSession {
    /// The russh client handle.
    handle: client::Handle<ClientHandler>,
    /// The active shell channel (set after `open_shell`).
    channel: Option<ChannelHandle>,
    /// List of bastion server IDs this session depends on (proxy_chain).
    /// Used for reference counting cleanup when session disconnects.
    /// Empty for direct connections, contains [bastion_id] for single-hop,
    /// [innermost_bastion, ..., outermost_bastion] for multi-hop.
    pub proxy_chain: Vec<String>,
}

impl SshSession {
    /// Connects to an SSH server. Authentication must be done separately.
    pub async fn connect(host: &str, port: u16) -> Result<Self, SshError> {
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            keepalive_interval: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        });

        let handler = ClientHandler;
        let handle = client::connect(config, (host, port), handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            handle,
            channel: None,
            proxy_chain: Vec::new(),
        })
    }

    /// Connects to an SSH server via a bastion host using direct-tcpip tunneling.
    /// The bastion handle must already be authenticated.
    pub async fn connect_via_proxy(
        bastion_handle: &client::Handle<ClientHandler>,
        target_host: &str,
        target_port: u16,
    ) -> Result<Self, SshError> {
        // Open direct-tcpip channel through bastion to target
        let channel = bastion_handle
            .channel_open_direct_tcpip(target_host, target_port as u32, "127.0.0.1", 0)
            .await
            .map_err(|e| SshError::ChannelError(format!("Failed to open direct-tcpip channel: {}", e)))?;

        // Extract the channel as a stream and establish SSH connection
        let stream = channel.into_stream();
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            keepalive_interval: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        });

        let handler = ClientHandler;
        let handle = client::connect_stream(config, stream, handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(format!("Failed to connect through proxy: {}", e)))?;

        Ok(Self {
            handle,
            channel: None,
            proxy_chain: Vec::new(),
        })
    }

    /// Connects to an SSH server through a network proxy (SOCKS5/SOCKS4/HTTP CONNECT).
    /// The proxy provides the TCP transport; SSH handshake happens over the tunneled stream.
    pub async fn connect_via_network_proxy(
        proxy: &ProxyConfig,
        host: &str,
        port: u16,
    ) -> Result<Self, SshError> {
        let stream = proxy::connect_via_proxy(proxy, host, port).await?;

        let config = Arc::new(client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            keepalive_interval: Some(std::time::Duration::from_secs(30)),
            ..Default::default()
        });

        let handler = ClientHandler;
        let handle = client::connect_stream(config, stream, handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(format!("SSH via proxy: {}", e)))?;

        Ok(Self {
            handle,
            channel: None,
            proxy_chain: Vec::new(),
        })
    }

    /// Returns a mutable reference to the client handle for authentication.
    pub fn handle_mut(&mut self) -> &mut client::Handle<ClientHandler> {
        &mut self.handle
    }

    /// Returns a reference to the client handle (e.g. for opening SFTP channels).
    pub fn handle(&self) -> &client::Handle<ClientHandler> {
        &self.handle
    }

    /// Opens a shell channel with a pseudo-terminal.
    pub async fn open_shell(
        &mut self,
        app: tauri::AppHandle,
        session_id: String,
        cols: u32,
        rows: u32,
    ) -> Result<(), SshError> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::ChannelError(e.to_string()))?;

        channel
            .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
            .await
            .map_err(|e| SshError::ChannelError(e.to_string()))?;

        channel
            .request_shell(false)
            .await
            .map_err(|e| SshError::ChannelError(e.to_string()))?;

        let handle = spawn_channel_task(channel, app, session_id);
        self.channel = Some(handle);

        Ok(())
    }

    /// Writes user input to the shell channel. Non-blocking.
    pub fn write(&self, data: &[u8]) -> Result<(), SshError> {
        let ch = self.channel.as_ref().ok_or(SshError::AlreadyDisconnected)?;
        ch.cmd_tx
            .send(ChannelCommand::Write(data.to_vec()))
            .map_err(|_| SshError::AlreadyDisconnected)
    }

    /// Resizes the terminal. Non-blocking.
    pub fn resize(&self, cols: u32, rows: u32) -> Result<(), SshError> {
        let ch = self.channel.as_ref().ok_or(SshError::AlreadyDisconnected)?;
        ch.cmd_tx
            .send(ChannelCommand::Resize(cols, rows))
            .map_err(|_| SshError::AlreadyDisconnected)
    }

    /// Opens an SFTP subsystem channel on this SSH connection.
    pub async fn open_sftp(&self) -> Result<SftpHandle, SshError> {
        SftpHandle::open(&self.handle)
            .await
            .map_err(|e| SshError::ChannelError(e.to_string()))
    }

    /// Disconnects the SSH session and cleans up resources.
    /// Returns the proxy_chain so the caller can decrement reference counts.
    pub async fn disconnect(mut self) -> Result<Vec<String>, SshError> {
        // 1. Close the shell channel first
        if let Some(ch) = self.channel.take() {
            let _ = ch.cmd_tx.send(ChannelCommand::Close);
            // Wait up to 2s for graceful close, then abort
            if tokio::time::timeout(
                std::time::Duration::from_secs(2),
                ch.task_handle,
            ).await.is_err() {
                // Task didn't finish in time — it's already been dropped
            }
        }
        // 2. Send SSH disconnect message
        let _ = self.handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        // 3. Return proxy_chain for reference count cleanup
        Ok(self.proxy_chain)
    }
}
