//! Tauri IPC commands for network proxy CRUD operations.

use serde::Deserialize;
use tauri::State;

use crate::crypto::aes;
use crate::keychain;
use crate::state::AppState;
use crate::storage::models::Proxy;
use crate::storage::proxies;

/// Input for creating or updating a proxy.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyInput {
    pub name: String,
    pub proxy_type: String,
    pub host: String,
    pub port: i32,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default = "default_true")]
    pub tls_verify: bool,
    pub ca_cert_path: Option<String>,
    pub client_cert_path: Option<String>,
    pub client_key_path: Option<String>,
}

fn default_true() -> bool { true }

/// Lists all proxy configurations.
#[tauri::command]
pub fn proxy_list(state: State<'_, AppState>) -> Result<Vec<Proxy>, String> {
    proxies::list(&state.db)
}

/// Creates a new proxy configuration.
#[tauri::command]
pub fn proxy_create(
    state: State<'_, AppState>,
    input: ProxyInput,
) -> Result<Proxy, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().to_string();
    let keychain_key = proxy_password_key(&id);

    let mk = state.master_key.read().expect("master_key lock poisoned").clone();
    let cred = store_proxy_credential(input.password.as_deref(), &keychain_key, &mk);

    proxies::create(
        &state.db, &id, &input.name, &input.proxy_type, &input.host, input.port,
        input.username.as_deref(),
        cred.encrypted.as_deref(), cred.keychain_id.as_deref(),
        input.tls_enabled, input.tls_verify,
        input.ca_cert_path.as_deref(), input.client_cert_path.as_deref(), input.client_key_path.as_deref(),
        &now,
    )?;

    Ok(Proxy {
        id, name: input.name, proxy_type: input.proxy_type,
        host: input.host, port: input.port, username: input.username,
        password_enc: None, password_keychain_id: cred.keychain_id,
        tls_enabled: input.tls_enabled, tls_verify: input.tls_verify,
        ca_cert_path: input.ca_cert_path, client_cert_path: input.client_cert_path,
        client_key_path: input.client_key_path,
        created_at: now.clone(), updated_at: now,
    })
}

/// Updates an existing proxy configuration.
#[tauri::command]
pub fn proxy_update(
    state: State<'_, AppState>,
    id: String,
    input: ProxyInput,
) -> Result<Proxy, String> {
    let now = time::OffsetDateTime::now_utc().to_string();
    let keychain_key = proxy_password_key(&id);

    let mk = state.master_key.read().expect("master_key lock poisoned").clone();
    let cred = store_proxy_credential(input.password.as_deref(), &keychain_key, &mk);

    proxies::update(
        &state.db, &id, &input.name, &input.proxy_type, &input.host, input.port,
        input.username.as_deref(),
        cred.encrypted.as_deref(), cred.keychain_id.as_deref(),
        input.tls_enabled, input.tls_verify,
        input.ca_cert_path.as_deref(), input.client_cert_path.as_deref(), input.client_key_path.as_deref(),
        &now,
    )?;

    Ok(Proxy {
        id, name: input.name, proxy_type: input.proxy_type,
        host: input.host, port: input.port, username: input.username,
        password_enc: None, password_keychain_id: cred.keychain_id,
        tls_enabled: input.tls_enabled, tls_verify: input.tls_verify,
        ca_cert_path: input.ca_cert_path, client_cert_path: input.client_cert_path,
        client_key_path: input.client_key_path,
        created_at: String::new(), updated_at: now,
    })
}

/// Deletes a proxy configuration.
#[tauri::command]
pub fn proxy_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let _ = keychain::delete(&proxy_password_key(&id));
    let now = time::OffsetDateTime::now_utc().to_string();
    proxies::delete(&state.db, &id, &now)
}

/// Returns the password for a proxy (from keychain or encrypted fallback).
#[tauri::command]
pub fn proxy_get_password(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    if let Ok(pw) = keychain::get(&proxy_password_key(&id)) {
        if !pw.is_empty() {
            return Ok(pw);
        }
    }

    let proxy = proxies::get(&state.db, &id)?;
    let mk = state.master_key.read().expect("master_key lock poisoned");
    match (&*mk, proxy.password_enc) {
        (Some(key), Some(data)) => {
            aes::decrypt(key, &data)
                .map(|p| String::from_utf8(p).unwrap_or_default())
                .map_err(|e| e.to_string())
        }
        _ => Ok(String::new()),
    }
}

// ── Helpers ─────────────────────────────────────────────────

/// Generates a keychain key for a proxy password.
pub fn proxy_password_key(proxy_id: &str) -> String {
    format!("termex:proxy:password:{proxy_id}")
}

struct StoredProxyCred {
    keychain_id: Option<String>,
    encrypted: Option<Vec<u8>>,
}

fn store_proxy_credential(
    value: Option<&str>,
    keychain_key: &str,
    master_key: &Option<[u8; 32]>,
) -> StoredProxyCred {
    let text = match value.filter(|s| !s.is_empty()) {
        Some(t) => t,
        None => return StoredProxyCred { keychain_id: None, encrypted: None },
    };

    let keychain_id = match keychain::store(keychain_key, text) {
        Ok(()) => Some(keychain_key.to_string()),
        Err(_) => None,
    };

    let encrypted = master_key
        .as_ref()
        .and_then(|key| aes::encrypt(key, text.as_bytes()).ok());

    StoredProxyCred { keychain_id, encrypted }
}
