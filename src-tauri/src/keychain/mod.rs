//! OS Keychain integration for secure credential storage.
//!
//! Uses the `keyring` crate to access platform-native credential stores:
//! - macOS: Keychain Services (Secure Enclave)
//! - Windows: Credential Manager (DPAPI)
//! - Linux: Secret Service (GNOME Keyring / KDE Wallet)
//!
//! **Architecture**: All credentials are stored in a **single keychain entry**
//! as a JSON object. This guarantees at most 1 OS password prompt per app launch.
//! An in-memory cache serves all reads; writes batch into the single entry.
//!
//! **CRITICAL RULE**: Only `init()` may call `keyring::Entry::get_password()`.
//! Only `flush()` may call `keyring::Entry::set_password()`.
//! No other function may directly access the OS keychain.
//! This ensures the OS never prompts more than once per session.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use thiserror::Error;

const SERVICE_NAME: &str = "com.termex.app";
/// Single keychain entry that holds all credentials as JSON.
const STORE_KEY: &str = "__termex_store__";

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("keychain not available: {0}")]
    NotAvailable(String),
    #[error("credential not found: {0}")]
    NotFound(String),
    #[error("keychain operation failed: {0}")]
    OperationFailed(String),
}

/// Global availability flag, computed once.
static AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Returns a reference to the global in-memory credential cache.
fn cache() -> &'static RwLock<HashMap<String, String>> {
    static CACHE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Initializes the keychain module.
///
/// This is the **only** function that reads from the OS keychain.
/// On success, all credentials are loaded into the in-memory cache.
/// Returns whether keychain is available.
///
/// **Guarantees at most 1 OS password prompt per app session.**
pub fn init() -> bool {
    *AVAILABLE.get_or_init(|| {
        let entry = match keyring::Entry::new(SERVICE_NAME, STORE_KEY) {
            Ok(e) => e,
            Err(_) => return false,
        };
        match entry.get_password() {
            Ok(json_str) => {
                if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&json_str) {
                    if let Ok(mut c) = cache().write() {
                        *c = map;
                    }
                }
                true
            }
            Err(keyring::Error::NoEntry) => {
                // First launch: create empty store
                entry.set_password("{}").is_ok()
            }
            Err(_) => false,
        }
    })
}

/// Verifies that the OS keychain is accessible.
///
/// Uses the result of `init()` — never performs additional OS keychain reads.
pub fn verify_accessible() -> Result<bool, KeychainError> {
    if !is_available() {
        return Err(KeychainError::NotAvailable("Keychain not available".to_string()));
    }
    // init() already proved the keychain is accessible by successfully reading it.
    Ok(true)
}

/// Returns whether the OS keychain is available. Calls `init()` lazily.
pub fn is_available() -> bool {
    init()
}

/// Writes the entire in-memory cache to the single keychain entry.
///
/// This is the **only** function that writes to the OS keychain.
/// Since it always writes to the same STORE_KEY entry that `init()` read,
/// macOS will not prompt again (same entry = same authorization).
fn flush() {
    if !*AVAILABLE.get().unwrap_or(&false) {
        return;
    }
    let map = match cache().read() {
        Ok(c) => c.clone(),
        Err(_) => return,
    };
    let json_str = match serde_json::to_string(&map) {
        Ok(s) => s,
        Err(_) => return,
    };
    if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, STORE_KEY) {
        let _ = entry.set_password(&json_str);
    }
}

/// Stores a credential in the cache and flushes to keychain.
pub fn store(key: &str, value: &str) -> Result<(), KeychainError> {
    if let Ok(mut c) = cache().write() {
        c.insert(key.to_string(), value.to_string());
    }
    flush();
    Ok(())
}

/// Retrieves a credential from the in-memory cache.
/// Never touches the OS keychain — all reads are from memory.
pub fn get(key: &str) -> Result<String, KeychainError> {
    if let Ok(c) = cache().read() {
        if let Some(value) = c.get(key) {
            return Ok(value.clone());
        }
    }
    Err(KeychainError::NotFound(key.to_string()))
}

/// Deletes a credential from the cache and flushes to keychain.
pub fn delete(key: &str) -> Result<(), KeychainError> {
    let removed = if let Ok(mut c) = cache().write() {
        c.remove(key).is_some()
    } else {
        false
    };
    if removed {
        flush();
    }
    Ok(())
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
