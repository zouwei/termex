use std::sync::Arc;

use russh::client;
use russh_keys::key;

use super::SshError;

/// SSH client handler that accepts all host keys (MVP).
/// TODO: Implement known_hosts verification in a later step.
pub struct ClientHandler;

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = SshError;

    /// Called when the server sends its public key.
    /// MVP: accepts all keys. Will be replaced with known_hosts verification.
    async fn check_server_key(
        &mut self,
        _server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
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

    let result = handle
        .authenticate_publickey(username, Arc::new(key_pair))
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
