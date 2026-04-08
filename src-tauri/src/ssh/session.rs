use std::sync::Arc;

use russh::client;
use russh::keys::PublicKey;
use tokio_util::sync::CancellationToken;

use super::auth::{self as ssh_auth, CapturedHostKey, ClientHandler, ExitForwardRegistry};
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
    /// If post-target exit routing is active, the proxy URL to inject (e.g., "socks5://host:port").
    pub exit_proxy_url: Option<String>,
    /// Cancellation token for the exit proxy background task.
    pub exit_proxy_cancel: Option<CancellationToken>,
    /// Exit forward registry shared with ClientHandler for forwarded-tcpip bridging.
    pub exit_forward_registry: ExitForwardRegistry,
    /// Shared captured host key from the SSH handshake.
    captured_host_key: CapturedHostKey,
}

impl SshSession {
    /// Creates a new SshSession from a handle.
    fn from_handle(
        handle: client::Handle<ClientHandler>,
        exit_reg: ExitForwardRegistry,
        captured_key: CapturedHostKey,
    ) -> Self {
        Self {
            handle,
            channel: None,
            proxy_chain: Vec::new(),
            exit_proxy_url: None,
            exit_proxy_cancel: None,
            exit_forward_registry: exit_reg,
            captured_host_key: captured_key,
        }
    }

    /// Creates a ClientHandler with exit forward registry and host key capture.
    fn new_handler() -> (ClientHandler, ExitForwardRegistry, CapturedHostKey) {
        let reg = ssh_auth::new_exit_forward_registry();
        let captured = ssh_auth::new_captured_host_key();
        let mut handler = ClientHandler::with_host_key_capture(captured.clone());
        handler.exit_forward_registry = Some(reg.clone());
        (handler, reg, captured)
    }

    /// Returns the captured server public key from the SSH handshake.
    /// Available after `connect()` returns successfully.
    pub fn captured_host_key(&self) -> Option<PublicKey> {
        self.captured_host_key.lock().ok().and_then(|k| k.clone())
    }

    /// Creates a hardened SSH client config with strong algorithm preferences.
    ///
    /// Disables weak algorithms (hmac-sha1) per ISO 27001 A.8.24 and 等保 2.0.
    /// Preferred order: chacha20-poly1305 > aes256-gcm > aes256-ctr.
    fn ssh_config() -> Arc<client::Config> {
        use std::borrow::Cow;

        Arc::new(client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
            keepalive_interval: Some(std::time::Duration::from_secs(30)),
            preferred: russh::Preferred {
                // Strong MAC algorithms only — no hmac-sha1
                mac: Cow::Borrowed(&[
                    russh::mac::HMAC_SHA512_ETM,
                    russh::mac::HMAC_SHA256_ETM,
                    russh::mac::HMAC_SHA512,
                    russh::mac::HMAC_SHA256,
                ]),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    /// Connects to an SSH server. Authentication must be done separately.
    /// The server's host key is captured during handshake for TOFU verification.
    pub async fn connect(host: &str, port: u16) -> Result<Self, SshError> {
        let config = Self::ssh_config();
        let (handler, reg, captured) = Self::new_handler();
        let handle = client::connect(config, (host, port), handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(e.to_string()))?;

        Ok(Self::from_handle(handle, reg, captured))
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

        let stream = channel.into_stream();
        let config = Self::ssh_config();
        let (handler, reg, captured) = Self::new_handler();
        let handle = client::connect_stream(config, stream, handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(format!("Failed to connect through proxy: {}", e)))?;

        Ok(Self::from_handle(handle, reg, captured))
    }

    /// Connects to an SSH server through a network proxy (SOCKS5/SOCKS4/HTTP CONNECT).
    /// The proxy provides the TCP transport; SSH handshake happens over the tunneled stream.
    pub async fn connect_via_network_proxy(
        proxy: &ProxyConfig,
        host: &str,
        port: u16,
    ) -> Result<Self, SshError> {
        let stream = proxy::connect_via_proxy(proxy, host, port).await?;
        let config = Self::ssh_config();
        let (handler, reg, captured) = Self::new_handler();
        let handle = client::connect_stream(config, stream, handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(format!("SSH via proxy: {}", e)))?;

        Ok(Self::from_handle(handle, reg, captured))
    }

    /// Connects to a target host by tunneling through a network proxy that is itself
    /// reachable via an SSH bastion's direct-tcpip channel.
    ///
    /// Flow: bastion → direct-tcpip to proxy:port → proxy handshake → SSH to target.
    pub async fn connect_via_tunneled_proxy(
        bastion_handle: &client::Handle<ClientHandler>,
        proxy: &ProxyConfig,
        target_host: &str,
        target_port: u16,
    ) -> Result<Self, SshError> {
        // 1. Open direct-tcpip channel from bastion to the proxy server
        let channel = bastion_handle
            .channel_open_direct_tcpip(
                &proxy.host,
                proxy.port as u32,
                "127.0.0.1",
                0,
            )
            .await
            .map_err(|e| {
                SshError::ProxyFailed(format!(
                    "Failed to open tunnel to proxy {}:{}: {}",
                    proxy.host, proxy.port, e
                ))
            })?;

        // 2. Convert channel to a stream for the proxy handshake
        let channel_stream = channel.into_stream();

        // 3. Perform the proxy protocol handshake (SOCKS5/HTTP CONNECT/etc.)
        //    over the SSH-tunneled stream to reach the target
        let tunneled_stream = proxy::connect_via_proxy_on_stream(
            proxy,
            target_host,
            target_port,
            Box::new(channel_stream),
        )
        .await?;

        // 4. Run SSH handshake over the proxy-tunneled stream
        let config = Self::ssh_config();
        let (handler, reg, captured) = Self::new_handler();
        let handle = client::connect_stream(config, tunneled_stream, handler)
            .await
            .map_err(|e| {
                SshError::ConnectionFailed(format!("SSH via tunneled proxy: {}", e))
            })?;

        Ok(Self::from_handle(handle, reg, captured))
    }

    /// Connects to an SSH server over a pre-established async stream.
    /// Used when the transport has been set up by the chain connection engine.
    pub async fn connect_on_stream(
        stream: Box<dyn proxy::AsyncStream>,
    ) -> Result<Self, SshError> {
        let config = Self::ssh_config();
        let (handler, reg, captured) = Self::new_handler();
        let handle = client::connect_stream(config, stream, handler)
            .await
            .map_err(|e| SshError::ConnectionFailed(format!("SSH via stream: {}", e)))?;

        Ok(Self::from_handle(handle, reg, captured))
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

    /// Executes a single command via a separate exec channel (not the PTY shell).
    /// Returns (stdout, exit_code). Does not interfere with the user's terminal.
    pub async fn exec_command(&self, command: &str) -> Result<(String, u32), SshError> {
        use russh::ChannelMsg;

        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| SshError::ChannelError(e.to_string()))?;

        channel
            .exec(true, command)
            .await
            .map_err(|e| SshError::ChannelError(e.to_string()))?;

        let mut output = Vec::new();
        let mut exit_code = 0u32;
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
                Some(ChannelMsg::ExitStatus { exit_status }) => exit_code = exit_status,
                Some(ChannelMsg::Eof) | None => break,
                _ => {}
            }
        }
        Ok((String::from_utf8_lossy(&output).to_string(), exit_code))
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
        // 1. Cancel exit proxy if active
        if let Some(cancel) = self.exit_proxy_cancel.take() {
            cancel.cancel();
        }
        // 2. Close the shell channel first
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
        // 3. Send SSH disconnect message
        let _ = self.handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        // 4. Return proxy_chain for reference count cleanup
        Ok(self.proxy_chain)
    }
}
