//! OS Keychain integration for secure credential storage.
//!
//! Uses the `keyring` crate to access platform-native credential stores:
//! - macOS: Keychain Services (Secure Enclave)
//! - Windows: Credential Manager (DPAPI)
//! - Linux: Secret Service (GNOME Keyring / KDE Wallet)
//!
//! An in-memory cache avoids repeated keychain prompts. Credentials are loaded
//! once on startup (via `preload`) and served from memory thereafter.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

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

/// Returns a reference to the global in-memory credential cache.
fn cache() -> &'static RwLock<HashMap<String, String>> {
    static CACHE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
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

/// Stores a credential in the OS keychain and updates the in-memory cache.
pub fn store(key: &str, value: &str) -> Result<(), KeychainError> {
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| KeychainError::NotAvailable(e.to_string()))?;
    entry
        .set_password(value)
        .map_err(|e| KeychainError::OperationFailed(e.to_string()))?;
    // Update cache
    if let Ok(mut c) = cache().write() {
        c.insert(key.to_string(), value.to_string());
    }
    Ok(())
}

/// Retrieves a credential. Returns from in-memory cache if available,
/// otherwise reads from OS keychain and caches the result.
pub fn get(key: &str) -> Result<String, KeychainError> {
    // Check cache first
    if let Ok(c) = cache().read() {
        if let Some(value) = c.get(key) {
            return Ok(value.clone());
        }
    }
    // Cache miss: read from OS keychain
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| KeychainError::NotAvailable(e.to_string()))?;
    let value = entry
        .get_password()
        .map_err(|e| KeychainError::NotFound(e.to_string()))?;
    // Store in cache for future reads
    if let Ok(mut c) = cache().write() {
        c.insert(key.to_string(), value.clone());
    }
    Ok(value)
}

/// Deletes a credential from both the in-memory cache and the OS keychain.
pub fn delete(key: &str) -> Result<(), KeychainError> {
    // Remove from cache
    if let Ok(mut c) = cache().write() {
        c.remove(key);
    }
    // Delete from OS keychain
    let entry = keyring::Entry::new(SERVICE_NAME, key)
        .map_err(|e| KeychainError::NotAvailable(e.to_string()))?;
    // Ignore "not found" errors on delete
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeychainError::OperationFailed(e.to_string())),
    }
}

/// Batch-loads credentials from the OS keychain into the in-memory cache.
/// Call once on startup to avoid repeated keychain password prompts.
pub fn preload(keys: &[String]) {
    let mut c = match cache().write() {
        Ok(c) => c,
        Err(_) => return,
    };
    for key in keys {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, key) {
            if let Ok(value) = entry.get_password() {
                c.insert(key.clone(), value);
            }
        }
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
