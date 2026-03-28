//! OS Keychain integration for secure credential storage.
//!
//! Uses the `keyring` crate to access platform-native credential stores:
//! - macOS: Keychain Services (Secure Enclave)
//! - Windows: Credential Manager (DPAPI)
//! - Linux: Secret Service (GNOME Keyring / KDE Wallet)

use thiserror::Error;

const SERVICE_NAME: &str = "com.termex.app";

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("keychain not available: {0}")]
    NotAvailable(String),
    #[error("credential not found: {0}")]
    NotFound(String),
    #[error("keychain operation failed: {0}")]
    OperationFailed(String),
}

/// Checks whether the OS keychain is available by doing a real store+delete probe.
pub fn is_available() -> bool {
    let entry = match keyring::Entry::new(SERVICE_NAME, "__termex_probe__") {
        Ok(e) => e,
        Err(_) => return false,
    };
    // Attempt a real write — on headless Linux this will fail with DBus errors
    if entry.set_password("probe").is_err() {
        return false;
    }
    let _ = entry.delete_credential();
    true
}

/// Stores a credential in the OS keychain.
pub fn store(key: &str, value: &str) -> Result<(), KeychainError> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| KeychainError::NotAvailable(e.to_string()))?;
    entry
        .set_password(value)
        .map_err(|e| KeychainError::OperationFailed(e.to_string()))
}

/// Retrieves a credential from the OS keychain.
pub fn get(key: &str) -> Result<String, KeychainError> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| KeychainError::NotAvailable(e.to_string()))?;
    entry
        .get_password()
        .map_err(|e| KeychainError::NotFound(e.to_string()))
}

/// Deletes a credential from the OS keychain.
pub fn delete(key: &str) -> Result<(), KeychainError> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| KeychainError::NotAvailable(e.to_string()))?;
    // Ignore "not found" errors on delete
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeychainError::OperationFailed(e.to_string())),
    }
}

/// Generates a keychain key for an SSH server password.
pub fn ssh_password_key(server_id: &str) -> String {
    format!("termex:ssh:password:{server_id}")
}

/// Generates a keychain key for an SSH passphrase.
pub fn ssh_passphrase_key(server_id: &str) -> String {
    format!("termex:ssh:passphrase:{server_id}")
}

/// Generates a keychain key for an AI provider API key.
pub fn ai_apikey_key(provider_id: &str) -> String {
    format!("termex:ai:apikey:{provider_id}")
}
