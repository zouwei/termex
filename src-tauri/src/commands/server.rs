use serde::{Deserialize, Serialize};
use tauri::State;

use crate::crypto::aes;
use crate::keychain;
use crate::state::AppState;
use crate::storage::models::{AuthType, ChainHopInput, Server, ServerGroup};

// ── Input types ────────────────────────────────────────────────

/// Input for creating or updating a server.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i32,
    pub username: String,
    pub auth_type: AuthType,
    /// Plaintext password — encrypted before storage, never persisted as-is.
    pub password: Option<String>,
    pub key_path: Option<String>,
    /// Plaintext passphrase — encrypted before storage.
    pub passphrase: Option<String>,
    pub group_id: Option<String>,
    pub proxy_id: Option<String>,
    pub network_proxy_id: Option<String>,
    pub startup_cmd: Option<String>,
    #[serde(default = "default_encoding")]
    pub encoding: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_tmux_mode")]
    pub tmux_mode: String,
    #[serde(default = "default_tmux_close_action")]
    pub tmux_close_action: String,
    #[serde(default)]
    pub git_sync_enabled: bool,
    #[serde(default = "default_git_sync_mode")]
    pub git_sync_mode: String,
    pub git_sync_local_path: Option<String>,
    pub git_sync_remote_path: Option<String>,
    /// Connection chain hops (replaces proxy_id/network_proxy_id for V10+).
    #[serde(default)]
    pub chain: Vec<ChainHopInput>,
}

fn default_tmux_mode() -> String {
    "disabled".into()
}
fn default_tmux_close_action() -> String {
    "detach".into()
}

fn default_port() -> i32 {
    22
}
fn default_encoding() -> String {
    "UTF-8".into()
}
fn default_git_sync_mode() -> String {
    "notify".into()
}

/// Input for creating or updating a group.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    pub name: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    pub parent_id: Option<String>,
}

fn default_color() -> String {
    "#6366f1".into()
}
fn default_icon() -> String {
    "folder".into()
}

/// Input for reordering items.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderItem {
    pub id: String,
    pub sort_order: i32,
}

// ── Server commands ────────────────────────────────────────────

/// Lists all servers with their group info and connection chains.
#[tauri::command]
pub fn server_list(state: State<'_, AppState>) -> Result<Vec<Server>, String> {
    let mut servers: Vec<Server> = state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, host, port, username, auth_type, password_enc, key_path,
                        passphrase_enc, group_id, sort_order, proxy_id, startup_cmd,
                        encoding, tags, last_connected, created_at, updated_at, network_proxy_id,
                        tmux_mode, tmux_close_action,
                        git_sync_enabled, git_sync_mode, git_sync_local_path, git_sync_remote_path
                 FROM servers ORDER BY sort_order, name",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    let tags_json: Option<String> = row.get(14)?;
                    let tags: Vec<String> = tags_json
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default();
                    let auth_str: String = row.get(5)?;
                    Ok(Server {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        host: row.get(2)?,
                        port: row.get(3)?,
                        username: row.get(4)?,
                        auth_type: AuthType::from_str(&auth_str)
                            .unwrap_or(AuthType::Password),
                        password_enc: row.get(6)?,
                        key_path: row.get(7)?,
                        passphrase_enc: row.get(8)?,
                        group_id: row.get(9)?,
                        sort_order: row.get(10)?,
                        proxy_id: row.get(11)?,
                        network_proxy_id: row.get(18)?,
                        startup_cmd: row.get(12)?,
                        encoding: row.get(13)?,
                        tags,
                        tmux_mode: row.get::<_, Option<String>>(19)?.unwrap_or_else(|| "disabled".into()),
                        tmux_close_action: row.get::<_, Option<String>>(20)?.unwrap_or_else(|| "detach".into()),
                        git_sync_enabled: row.get::<_, Option<bool>>(21)?.unwrap_or(false),
                        git_sync_mode: row.get::<_, Option<String>>(22)?.unwrap_or_else(|| "notify".into()),
                        git_sync_local_path: row.get(23)?,
                        git_sync_remote_path: row.get(24)?,
                        chain: Vec::new(), // populated below
                        last_connected: row.get(15)?,
                        created_at: row.get(16)?,
                        updated_at: row.get(17)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
        .map_err(|e| e.to_string())?;

    // Load connection chains for all servers
    for server in &mut servers {
        server.chain = crate::storage::chain::list(&state.db, &server.id).unwrap_or_default();
    }

    Ok(servers)
}

/// Creates a new server connection.
#[tauri::command]
pub fn server_create(
    state: State<'_, AppState>,
    input: ServerInput,
) -> Result<Server, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().to_string();
    let tags_json = serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".into());

    // Store credentials in OS keychain + encrypted fallback
    let mk = state.master_key.read().expect("master_key lock poisoned").clone();
    let pw = store_credential(
        input.password.as_deref(),
        &keychain::ssh_password_key(&id),
        &mk,
    );
    let pp = store_credential(
        input.passphrase.as_deref(),
        &keychain::ssh_passphrase_key(&id),
        &mk,
    );

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO servers (id, name, host, port, username, auth_type,
                    password_enc, password_keychain_id, key_path,
                    passphrase_enc, passphrase_keychain_id, group_id, sort_order,
                    proxy_id, network_proxy_id, startup_cmd, encoding, tags,
                    tmux_mode, tmux_close_action,
                    git_sync_enabled, git_sync_mode, git_sync_local_path, git_sync_remote_path,
                    created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,
                         ?19,?20,?21,?22,?23,?24,?25,?26)",
                rusqlite::params![
                    id,
                    input.name,
                    input.host,
                    input.port,
                    input.username,
                    input.auth_type.as_str(),
                    pw.encrypted,
                    pw.keychain_id,
                    input.key_path,
                    pp.encrypted,
                    pp.keychain_id,
                    input.group_id,
                    0,
                    input.proxy_id,
                    input.network_proxy_id,
                    input.startup_cmd,
                    input.encoding,
                    tags_json,
                    input.tmux_mode,
                    input.tmux_close_action,
                    input.git_sync_enabled,
                    input.git_sync_mode,
                    input.git_sync_local_path,
                    input.git_sync_remote_path,
                    now,
                    now,
                ],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    // Save connection chain if provided
    if !input.chain.is_empty() {
        crate::storage::chain::save(&state.db, &id, &input.chain)
            .map_err(|e| e.to_string())?;
    }

    // Load saved chain for response
    let chain = crate::storage::chain::list(&state.db, &id).unwrap_or_default();

    Ok(Server {
        id,
        name: input.name,
        host: input.host,
        port: input.port,
        username: input.username,
        auth_type: input.auth_type,
        password_enc: None,
        key_path: input.key_path,
        passphrase_enc: None,
        group_id: input.group_id,
        sort_order: 0,
        proxy_id: input.proxy_id,
        network_proxy_id: input.network_proxy_id,
        startup_cmd: input.startup_cmd,
        encoding: input.encoding,
        tags: input.tags,
        tmux_mode: input.tmux_mode,
        tmux_close_action: input.tmux_close_action,
        git_sync_enabled: input.git_sync_enabled,
        git_sync_mode: input.git_sync_mode,
        git_sync_local_path: input.git_sync_local_path,
        git_sync_remote_path: input.git_sync_remote_path,
        chain,
        last_connected: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Updates an existing server.
#[tauri::command]
pub fn server_update(
    state: State<'_, AppState>,
    id: String,
    input: ServerInput,
) -> Result<Server, String> {
    let now = time::OffsetDateTime::now_utc().to_string();
    let tags_json = serde_json::to_string(&input.tags).unwrap_or_else(|_| "[]".into());

    // Update keychain + encrypted fallback (only if provided / non-empty)
    let mk = state.master_key.read().expect("master_key lock poisoned").clone();
    let pw = store_credential(
        input.password.as_deref(),
        &keychain::ssh_password_key(&id),
        &mk,
    );
    let pp = store_credential(
        input.passphrase.as_deref(),
        &keychain::ssh_passphrase_key(&id),
        &mk,
    );

    state
        .db
        .with_conn(|conn| {
            let affected = conn.execute(
                "UPDATE servers SET name=?1, host=?2, port=?3, username=?4, auth_type=?5,
                    password_enc=COALESCE(?6, password_enc),
                    password_keychain_id=COALESCE(?7, password_keychain_id),
                    key_path=?8,
                    passphrase_enc=COALESCE(?9, passphrase_enc),
                    passphrase_keychain_id=COALESCE(?10, passphrase_keychain_id),
                    group_id=?11,
                    proxy_id=?12, network_proxy_id=?13,
                    startup_cmd=?14, encoding=?15, tags=?16,
                    tmux_mode=?17, tmux_close_action=?18,
                    git_sync_enabled=?19, git_sync_mode=?20,
                    git_sync_local_path=?21, git_sync_remote_path=?22,
                    updated_at=?23
                 WHERE id=?24",
                rusqlite::params![
                    input.name,
                    input.host,
                    input.port,
                    input.username,
                    input.auth_type.as_str(),
                    pw.encrypted,
                    pw.keychain_id,
                    input.key_path,
                    pp.encrypted,
                    pp.keychain_id,
                    input.group_id,
                    input.proxy_id,
                    input.network_proxy_id,
                    input.startup_cmd,
                    input.encoding,
                    tags_json,
                    input.tmux_mode,
                    input.tmux_close_action,
                    input.git_sync_enabled,
                    input.git_sync_mode,
                    input.git_sync_local_path,
                    input.git_sync_remote_path,
                    now,
                    id,
                ],
            )?;
            if affected == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    // Save connection chain (always overwrite — even empty chain clears old hops)
    crate::storage::chain::save(&state.db, &id, &input.chain)
        .map_err(|e| e.to_string())?;

    // Load saved chain for response
    let chain = crate::storage::chain::list(&state.db, &id).unwrap_or_default();

    Ok(Server {
        id,
        name: input.name,
        host: input.host,
        port: input.port,
        username: input.username,
        auth_type: input.auth_type,
        password_enc: None,
        key_path: input.key_path,
        passphrase_enc: None,
        group_id: input.group_id,
        sort_order: 0,
        proxy_id: input.proxy_id,
        network_proxy_id: input.network_proxy_id,
        startup_cmd: input.startup_cmd,
        encoding: input.encoding,
        tags: input.tags,
        tmux_mode: input.tmux_mode,
        tmux_close_action: input.tmux_close_action,
        git_sync_enabled: input.git_sync_enabled,
        git_sync_mode: input.git_sync_mode,
        git_sync_local_path: input.git_sync_local_path,
        git_sync_remote_path: input.git_sync_remote_path,
        chain,
        last_connected: None,
        created_at: String::new(),
        updated_at: now,
    })
}

/// Deletes a server by ID and removes its keychain credentials.
/// Also cleans up any proxy_id references to this server.
#[tauri::command]
pub fn server_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    // Clean up keychain entries
    let _ = keychain::delete(&keychain::ssh_password_key(&id));
    let _ = keychain::delete(&keychain::ssh_passphrase_key(&id));

    let now = time::OffsetDateTime::now_utc().to_string();

    state
        .db
        .with_conn(|conn| {
            // Clear proxy_id references to this server (servers using it as bastion)
            conn.execute(
                "UPDATE servers SET proxy_id = NULL, updated_at = ?1 WHERE proxy_id = ?2",
                rusqlite::params![now, id],
            )?;
            // Delete the server itself
            conn.execute("DELETE FROM servers WHERE id = ?1", rusqlite::params![id])?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Returns credentials for a server (from keychain or legacy encrypted fields).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCredentials {
    pub password: String,
    pub passphrase: String,
}

#[tauri::command]
pub fn server_get_credentials(
    state: State<'_, AppState>,
    id: String,
) -> Result<ServerCredentials, String> {
    // Try keychain first
    let password = keychain::get(&keychain::ssh_password_key(&id)).unwrap_or_default();
    let passphrase = keychain::get(&keychain::ssh_passphrase_key(&id)).unwrap_or_default();

    // If keychain returned values, use them
    if !password.is_empty() || !passphrase.is_empty() {
        return Ok(ServerCredentials { password, passphrase });
    }

    // Fallback: read legacy encrypted fields from DB
    let (password_enc, passphrase_enc): (Option<Vec<u8>>, Option<Vec<u8>>) = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT password_enc, passphrase_enc FROM servers WHERE id = ?1",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
        })
        .map_err(|e| e.to_string())?;

    let mk = state.master_key.read().expect("master_key lock poisoned");

    let password = decrypt_legacy(&mk, password_enc);
    let passphrase = decrypt_legacy(&mk, passphrase_enc);

    Ok(ServerCredentials { password, passphrase })
}

/// Updates the last_connected timestamp for a server.
#[tauri::command]
pub fn server_touch(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let now = time::OffsetDateTime::now_utc().to_string();
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE servers SET last_connected = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Reorders servers/groups by updating sort_order.
#[tauri::command]
pub fn server_reorder(
    state: State<'_, AppState>,
    orders: Vec<ReorderItem>,
) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            for item in &orders {
                conn.execute(
                    "UPDATE servers SET sort_order = ?1 WHERE id = ?2",
                    rusqlite::params![item.sort_order, item.id],
                )?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())
}

// ── Group commands ─────────────────────────────────────────────

/// Lists all server groups.
#[tauri::command]
pub fn group_list(state: State<'_, AppState>) -> Result<Vec<ServerGroup>, String> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, color, icon, parent_id, sort_order, created_at, updated_at
                 FROM groups ORDER BY sort_order, name",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(ServerGroup {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        color: row.get(2)?,
                        icon: row.get(3)?,
                        parent_id: row.get(4)?,
                        sort_order: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
        .map_err(|e| e.to_string())
}

/// Creates a new server group.
#[tauri::command]
pub fn group_create(
    state: State<'_, AppState>,
    input: GroupInput,
) -> Result<ServerGroup, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().to_string();

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO groups (id, name, color, icon, parent_id, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
                rusqlite::params![id, input.name, input.color, input.icon, input.parent_id, now],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(ServerGroup {
        id,
        name: input.name,
        color: input.color,
        icon: input.icon,
        parent_id: input.parent_id,
        sort_order: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// Updates an existing group.
#[tauri::command]
pub fn group_update(
    state: State<'_, AppState>,
    id: String,
    input: GroupInput,
) -> Result<ServerGroup, String> {
    let now = time::OffsetDateTime::now_utc().to_string();

    state
        .db
        .with_conn(|conn| {
            let affected = conn.execute(
                "UPDATE groups SET name=?1, color=?2, icon=?3, parent_id=?4, updated_at=?5
                 WHERE id=?6",
                rusqlite::params![input.name, input.color, input.icon, input.parent_id, now, id],
            )?;
            if affected == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(ServerGroup {
        id,
        name: input.name,
        color: input.color,
        icon: input.icon,
        parent_id: input.parent_id,
        sort_order: 0,
        created_at: String::new(),
        updated_at: now,
    })
}

/// Deletes a group. Servers in the group become ungrouped (SET NULL).
#[tauri::command]
pub fn group_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            conn.execute("DELETE FROM groups WHERE id = ?1", rusqlite::params![id])?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Reorders groups by updating sort_order.
#[tauri::command]
pub fn group_reorder(
    state: State<'_, AppState>,
    orders: Vec<ReorderItem>,
) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            for item in &orders {
                conn.execute(
                    "UPDATE groups SET sort_order = ?1 WHERE id = ?2",
                    rusqlite::params![item.sort_order, item.id],
                )?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())
}

// ── Helpers ────────────────────────────────────────────────────

/// Result of storing a credential: keychain ID and/or encrypted fallback blob.
struct StoredCredential {
    keychain_id: Option<String>,
    encrypted: Option<Vec<u8>>,
}

/// Stores a credential in OS keychain + AES-256-GCM encrypted fallback.
/// Always produces an encrypted fallback so credentials survive keychain issues.
fn store_credential(
    value: Option<&str>,
    keychain_key: &str,
    master_key: &Option<zeroize::Zeroizing<[u8; 32]>>,
) -> StoredCredential {
    let text = match value.filter(|s| !s.is_empty()) {
        Some(t) => t,
        None => return StoredCredential { keychain_id: None, encrypted: None },
    };

    // Try keychain first
    let keychain_id = match keychain::store(keychain_key, text) {
        Ok(()) => Some(keychain_key.to_string()),
        Err(_) => None,
    };

    // Always store encrypted fallback
    let encrypted = master_key
        .as_ref()
        .and_then(|key| aes::encrypt(key, text.as_bytes()).ok());

    StoredCredential { keychain_id, encrypted }
}

/// Legacy helper for backward compat (returns only keychain_id).
fn store_to_keychain(value: Option<&str>, keychain_key: &str) -> Option<String> {
    let text = value.filter(|s| !s.is_empty())?;
    match keychain::store(keychain_key, text) {
        Ok(()) => Some(keychain_key.to_string()),
        Err(_) => None,
    }
}

/// Decrypts a legacy encrypted field (pre-keychain migration).
fn decrypt_legacy(mk: &std::sync::RwLockReadGuard<'_, Option<zeroize::Zeroizing<[u8; 32]>>>, enc: Option<Vec<u8>>) -> String {
    match (&**mk, enc) {
        (Some(key), Some(data)) => {
            aes::decrypt(key, &data)
                .map(|p| String::from_utf8(p).unwrap_or_default())
                .unwrap_or_default()
        }
        (None, Some(data)) => String::from_utf8(data).unwrap_or_default(),
        _ => String::new(),
    }
}
