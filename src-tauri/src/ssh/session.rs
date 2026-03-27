use std::sync::Arc;

use russh::client;

use super::auth::ClientHandler;
use super::channel::{ChannelCommand, ChannelHandle, spawn_channel_task};
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
    pub async fn disconnect(self) -> Result<(), SshError> {
        if let Some(ch) = self.channel {
            let _ = ch.cmd_tx.send(ChannelCommand::Close);
            ch.task_handle.abort();
        }
        self.handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await
            .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;
        Ok(())
    }
}
