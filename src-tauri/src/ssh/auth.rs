use std::collections::HashMap;
use std::sync::Arc;

use russh::client::{self, Msg, Session};
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::PublicKey;
use russh::{Channel, ChannelMsg};
use tokio::sync::RwLock;

use super::reverse_forward::{
    parse_sync_request, SharedReverseForwardRegistry, SyncAction, HTTP_200,
};
use super::SshError;

/// Registry for exit proxy forwarded ports.
/// Maps "address:port" to a local TCP port where the SOCKS5 relay listens.
pub type ExitForwardRegistry = Arc<RwLock<HashMap<String, u16>>>;

/// Creates a new empty exit forward registry.
pub fn new_exit_forward_registry() -> ExitForwardRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Shared container for the captured server public key.
/// Used by both ClientHandler (writes) and SshSession (reads) for post-connect TOFU verification.
pub type CapturedHostKey = Arc<std::sync::Mutex<Option<PublicKey>>>;

/// Creates a new empty captured host key container.
pub fn new_captured_host_key() -> CapturedHostKey {
    Arc::new(std::sync::Mutex::new(None))
}

/// SSH client handler with TOFU host key capture.
/// Also handles reverse port forwarding channels for Git Auto Sync and exit proxy.
pub struct ClientHandler {
    /// Shared reverse forward registry for Git Sync.
    pub reverse_registry: Option<SharedReverseForwardRegistry>,
    /// Exit proxy forward registry: remote address:port → local SOCKS5 port.
    pub exit_forward_registry: Option<ExitForwardRegistry>,
    /// Tauri app handle for emitting events.
    pub app_handle: Option<tauri::AppHandle>,
    /// Shared captured server public key from the SSH handshake.
    pub captured_host_key: CapturedHostKey,
}

impl ClientHandler {
    /// Creates a basic handler.
    pub fn new() -> Self {
        Self {
            reverse_registry: None,
            exit_forward_registry: None,
            app_handle: None,
            captured_host_key: new_captured_host_key(),
        }
    }

    /// Creates a handler with a shared host key container.
    pub fn with_host_key_capture(captured: CapturedHostKey) -> Self {
        Self {
            reverse_registry: None,
            exit_forward_registry: None,
            app_handle: None,
            captured_host_key: captured,
        }
    }

    /// Creates a handler with reverse forwarding support.
    pub fn with_reverse_forward(
        registry: SharedReverseForwardRegistry,
        app_handle: tauri::AppHandle,
    ) -> Self {
        Self {
            reverse_registry: Some(registry),
            exit_forward_registry: None,
            app_handle: Some(app_handle),
            captured_host_key: new_captured_host_key(),
        }
    }
}

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = SshError;

    /// Called when the server sends its public key.
    /// Captures the key for post-connect TOFU verification and accepts the connection.
    /// The actual verification happens in the command layer after connect returns.
    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // Capture the server's public key for TOFU verification
        if let Ok(mut key) = self.captured_host_key.lock() {
            *key = Some(server_public_key.clone());
        }
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
        let key = format!("{}:{}", connected_address, connected_port);
        eprintln!(">>> [FORWARDED-TCPIP] Received: {} (from {}:{})", key, _originator_address, _originator_port);

        // Check exit proxy registry first
        if let Some(exit_reg) = &self.exit_forward_registry {
            let reg = exit_reg.read().await;
            eprintln!(">>> [FORWARDED-TCPIP] Registry has {} entries: {:?}", reg.len(), reg.keys().collect::<Vec<_>>());
            if let Some(&local_port) = reg.get(&key) {
                drop(reg);
                eprintln!(">>> [FORWARDED-TCPIP] Matched! Bridging to local:{}", local_port);
                // Bridge this channel to the local SOCKS5 server
                tokio::spawn(async move {
                    bridge_forwarded_to_local(channel, local_port).await;
                });
                return Ok(());
            }
            eprintln!(">>> [FORWARDED-TCPIP] No match for key: {}", key);
        } else {
            eprintln!(">>> [FORWARDED-TCPIP] No exit_forward_registry on handler");
        }

        // Fall through to Git Sync reverse forward registry
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
///
/// For RSA keys, tries multiple hash algorithms in order of preference:
/// `rsa-sha2-256` → `rsa-sha2-512` → `ssh-rsa` (SHA-1 legacy).
/// This ensures compatibility with both modern (OpenSSH 8.2+) and older servers.
pub async fn auth_key_data(
    handle: &mut client::Handle<ClientHandler>,
    username: &str,
    key_data: &str,
    passphrase: Option<&str>,
) -> Result<(), SshError> {
    let key_pair = russh_keys::decode_secret_key(key_data, passphrase)?;
    let key = Arc::new(key_pair);

    let is_rsa = key.algorithm().is_rsa();
    if is_rsa {
        // Try SHA-256 first (most widely supported modern algorithm),
        // then SHA-512, then legacy SHA-1 for old servers.
        let hash_algs: &[Option<russh::keys::HashAlg>] = &[
            Some(russh::keys::HashAlg::Sha256),
            Some(russh::keys::HashAlg::Sha512),
            None, // ssh-rsa (SHA-1) for legacy servers
        ];
        for hash_alg in hash_algs {
            let key_with_hash = PrivateKeyWithHashAlg::new(Arc::clone(&key), *hash_alg)
                .map_err(|e| SshError::AuthFailed(e.to_string()))?;
            match handle.authenticate_publickey(username, key_with_hash).await {
                Ok(true) => return Ok(()),
                Ok(false) => continue,
                Err(e) => return Err(SshError::AuthFailed(e.to_string())),
            }
        }
        return Err(SshError::AuthFailed("key rejected".into()));
    }

    // Non-RSA keys (Ed25519, ECDSA): no hash algorithm needed
    let key_with_hash = PrivateKeyWithHashAlg::new(key, None)
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

/// Bridges a forwarded-tcpip SSH channel to a local TCP port (exit proxy SOCKS5 server).
async fn bridge_forwarded_to_local(mut channel: Channel<Msg>, local_port: u16) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let Ok(mut tcp) = TcpStream::connect(format!("127.0.0.1:{}", local_port)).await else {
        let _ = channel.close().await;
        return;
    };

    let (mut tcp_rd, mut tcp_wr) = tcp.split();
    let mut buf = vec![0u8; 32768];

    loop {
        tokio::select! {
            // SSH channel → local TCP
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        if tcp_wr.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::Eof) | None => break,
                    _ => {}
                }
            }
            // Local TCP → SSH channel
            result = tcp_rd.read(&mut buf) => {
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if channel.data(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }

    let _ = channel.close().await;
}
