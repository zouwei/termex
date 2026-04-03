use std::sync::Arc;

use russh::client::{self, Msg, Session};
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::PublicKey;
use russh::{Channel, ChannelMsg};

use super::reverse_forward::{
    parse_sync_request, SharedReverseForwardRegistry, SyncAction, HTTP_200,
};
use super::SshError;

/// SSH client handler that accepts all host keys (MVP).
/// Also handles reverse port forwarding channels for Git Auto Sync.
pub struct ClientHandler {
    /// Shared reverse forward registry for Git Sync.
    pub reverse_registry: Option<SharedReverseForwardRegistry>,
    /// Tauri app handle for emitting events.
    pub app_handle: Option<tauri::AppHandle>,
}

impl ClientHandler {
    /// Creates a basic handler without reverse forwarding support.
    pub fn new() -> Self {
        Self {
            reverse_registry: None,
            app_handle: None,
        }
    }

    /// Creates a handler with reverse forwarding support.
    pub fn with_reverse_forward(
        registry: SharedReverseForwardRegistry,
        app_handle: tauri::AppHandle,
    ) -> Self {
        Self {
            reverse_registry: Some(registry),
            app_handle: Some(app_handle),
        }
    }
}

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = SshError;

    /// Called when the server sends its public key.
    /// MVP: accepts all keys. Will be replaced with known_hosts verification.
    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    /// Called when the server opens a forwarded-tcpip channel (reverse port forwarding).
    /// Reads the HTTP request from the remote curl, parses it, and emits a Tauri event.
    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let registry = match &self.reverse_registry {
            Some(r) => r.clone(),
            None => return Ok(()),
        };
        let app_handle = match &self.app_handle {
            Some(h) => h.clone(),
            None => return Ok(()),
        };
        let address = connected_address.to_string();
        let port = connected_port;

        // Spawn a task to handle the channel data asynchronously
        tokio::spawn(async move {
            let reg = registry.read().await;
            let server_id = match reg.lookup(&address, port) {
                Some(entry) => entry.server_id.clone(),
                None => return,
            };
            drop(reg);

            // Read data from the channel
            let mut channel = channel;
            let mut buf = Vec::new();
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Data { data }) => {
                        buf.extend_from_slice(&data);
                        // Check if we have a complete HTTP request
                        if let Some(action) = parse_sync_request(&buf) {
                            // Send HTTP 200 response
                            let _ = channel.data(&HTTP_200[..]).await;
                            let _ = channel.eof().await;
                            let _ = channel.close().await;

                            match action {
                                SyncAction::PushDone => {
                                    use tauri::Emitter;
                                    let _ = app_handle.emit("git-sync://push-done", &server_id);
                                }
                            }
                            return;
                        }
                    }
                    Some(ChannelMsg::Eof) | None => {
                        let _ = channel.close().await;
                        return;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }
}

/// Authenticates with the SSH server using a password.
pub async fn auth_password(
    handle: &mut client::Handle<ClientHandler>,
    username: &str,
    password: &str,
) -> Result<(), SshError> {
    let result = handle
        .authenticate_password(username, password)
        .await
        .map_err(|e| SshError::AuthFailed(e.to_string()))?;

    if !result {
        return Err(SshError::AuthFailed("password rejected".into()));
    }
    Ok(())
}

/// Authenticates with the SSH server using a private key from content (bytes).
/// `key_data` is the raw key file content (PEM format).
pub async fn auth_key_data(
    handle: &mut client::Handle<ClientHandler>,
    username: &str,
    key_data: &str,
    passphrase: Option<&str>,
) -> Result<(), SshError> {
    let key_pair = russh_keys::decode_secret_key(key_data, passphrase)?;
    let key_with_hash = PrivateKeyWithHashAlg::new(Arc::new(key_pair), None)
        .map_err(|e| SshError::AuthFailed(e.to_string()))?;

    let result = handle
        .authenticate_publickey(username, key_with_hash)
        .await
        .map_err(|e| SshError::AuthFailed(e.to_string()))?;

    if !result {
        return Err(SshError::AuthFailed("key rejected".into()));
    }
    Ok(())
}

/// Authenticates with the SSH server using a private key file.
/// This is kept for backward compatibility but now calls `auth_key_data` internally.
pub async fn auth_key(
    handle: &mut client::Handle<ClientHandler>,
    username: &str,
    key_path: &str,
    passphrase: Option<&str>,
) -> Result<(), SshError> {
    // Try to read the key as a file first (for file paths)
    if let Ok(key_data) = std::fs::read_to_string(key_path) {
        return auth_key_data(handle, username, &key_data, passphrase).await;
    }

    // If file read fails, treat key_path as the key content directly
    // (this handles the case where the frontend passes key content instead of path)
    auth_key_data(handle, username, key_path, passphrase).await
}
