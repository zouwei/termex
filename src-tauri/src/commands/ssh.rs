use tauri::{AppHandle, Emitter, State};

use crate::crypto::aes;
use crate::keychain;
use crate::ssh::chain_connect::{
    self, ProxyHopInfo, ResolvedHop, ResolvedTarget, SshHopInfo,
};
use crate::ssh::host_key::{self, HostKeyVerifyResult};
use crate::ssh::proxy::{ProxyConfig, ProxyTlsConfig, ProxyType};
use crate::ssh::session::SshSession;
use crate::ssh::{auth, SshError};
use crate::state::AppState;
use crate::storage::models::{AuthType, ChainHop};

/// Connects to an SSH server and authenticates (without opening a shell).
/// Returns the session_id for subsequent operations.
/// Call `ssh_open_shell` after terminal UI is mounted to open the shell with correct dimensions.
#[tauri::command]
pub async fn ssh_connect(
    state: State<'_, AppState>,
    app: AppHandle,
    server_id: String,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let status_event = format!("ssh://status/{session_id}");

    // Load server details from database
    let server = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT host, port, username, auth_type, password_enc, key_path, passphrase_enc, proxy_id, network_proxy_id
                 FROM servers WHERE id = ?1",
                rusqlite::params![server_id],
                |row| {
                    Ok(ServerInfo {
                        host: row.get(0)?,
                        port: row.get(1)?,
                        username: row.get(2)?,
                        auth_type: row.get(3)?,
                        password_enc: row.get(4)?,
                        key_path: row.get(5)?,
                        passphrase_enc: row.get(6)?,
                        proxy_id: row.get(7)?,
                        network_proxy_id: row.get(8)?,
                        server_id: server_id.clone(),
                    })
                },
            )
        })
        .map_err(|e| e.to_string())?;

    // Emit connecting status
    let _ = app.emit(
        &status_event,
        serde_json::json!({"status": "connecting", "message": "connecting..."}),
    );

    // Audit: record connection attempt
    crate::audit::log(
        &state.db,
        crate::audit::AuditEvent::SshConnectAttempt {
            server_id: server_id.clone(),
            host: server.host.clone(),
        },
    );

    // Load connection chain from DB (V10+)
    let chain_hops = crate::storage::chain::list(&state.db, &server_id).unwrap_or_default();

    // Resolve hops: either from connection_chain table or legacy proxy_id/network_proxy_id
    let (pre_hops, post_hops) = if !chain_hops.is_empty() {
        resolve_chain_hops(&state, &chain_hops, &app, &status_event)?
    } else {
        // Legacy fallback: build chain from proxy_id + network_proxy_id
        let legacy_pre = build_legacy_pre_hops(&state, &server, &app, &status_event)?;
        (legacy_pre, Vec::new())
    };

    // Resolve target credentials
    let resolved_target = resolve_target_info(&state, &server)?;

    // Connect through the chain
    let result = chain_connect::connect_chain(
        state.inner(),
        &app,
        &status_event,
        pre_hops,
        resolved_target,
        post_hops,
    )
    .await
    .map_err(|e| {
        crate::audit::log(
            &state.db,
            crate::audit::AuditEvent::SshConnectFailed {
                server_id: server_id.clone(),
                error: e.to_string(),
            },
        );
        emit_error(&app, &status_event, &e)
    })?;

    let ssh_session = result.target_session;
    let post_hops = result.post_hops;

    // TOFU host key verification: check captured key against known_hosts
    if let Some(pubkey) = ssh_session.captured_host_key() {
        let verify_result = host_key::verify_host_key(
            &state.db, &server.host, server.port as u16, &pubkey,
        );
        match &verify_result {
            HostKeyVerifyResult::Trusted => {
                // Key matches — continue
            }
            HostKeyVerifyResult::NewHost { key_type, fingerprint } => {
                // First-time connection — auto-trust and continue
                // (silent TOFU: trust on first use without prompting)
                let _ = host_key::trust_host_key(
                    &state.db, &server.host, server.port as u16, &pubkey,
                );
                let _ = app.emit(
                    &status_event,
                    serde_json::json!({
                        "status": "host_key_trusted",
                        "message": format!("Host key fingerprint ({key_type}): {fingerprint}"),
                    }),
                );
            }
            HostKeyVerifyResult::KeyChanged { key_type, old_fingerprint, new_fingerprint } => {
                // Key has changed — potential MITM!
                // Emit event to frontend and wait for user decision
                let _ = app.emit(
                    "ssh://host-key-changed",
                    serde_json::json!({
                        "host": server.host,
                        "port": server.port,
                        "keyType": key_type,
                        "oldFingerprint": old_fingerprint,
                        "newFingerprint": new_fingerprint,
                        "sessionId": session_id,
                    }),
                );

                // Wait for user decision via a oneshot channel stored in state
                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                {
                    let mut pending = state.pending_host_key_decisions.write().await;
                    pending.insert(session_id.clone(), tx);
                }
                let accepted = tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    rx,
                )
                .await
                .unwrap_or(Ok(false))
                .unwrap_or(false);

                if !accepted {
                    // User rejected — disconnect
                    let _ = ssh_session.disconnect().await;
                    return Err(emit_error(&app, &status_event, &SshError::ConnectionFailed(
                        "Host key verification failed: key has changed. Connection rejected by user.".into(),
                    )));
                }

                // User accepted — update stored key
                let _ = host_key::remove_host_key(
                    &state.db, &server.host, server.port as u16,
                );
                let _ = host_key::trust_host_key(
                    &state.db, &server.host, server.port as u16, &pubkey,
                );
            }
        }
    }

    // Store session FIRST (needed for exit routing to access it by session_id)
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id.clone(), ssh_session);
    }

    // Set up post-target exit routing (if any) — resolve exit proxy URL
    if !post_hops.is_empty() {
        match chain_connect::setup_post_target_exit(
            &app,
            &session_id,
            &status_event,
            post_hops,
        )
        .await
        {
            Ok(exit_info) => {
                let mut sessions = state.sessions.write().await;
                if let Some(s) = sessions.get_mut(&session_id) {
                    s.exit_proxy_url = Some(exit_info.proxy_url);
                    s.exit_proxy_cancel = exit_info.cancel;
                }
            }
            Err(e) => {
                // Exit routing failed — remove session and report error
                let mut sessions = state.sessions.write().await;
                if let Some(s) = sessions.remove(&session_id) {
                    let _ = s.disconnect().await;
                }
                return Err(emit_error(&app, &status_event, &e));
            }
        }
    }

    // Emit authenticated status (shell not yet open)
    let _ = app.emit(
        &status_event,
        serde_json::json!({
            "status": "authenticated",
            "message": format!("{}@{}:{}", server.username, server.host, server.port),
        }),
    );

    // Audit: record successful connection
    crate::audit::log(
        &state.db,
        crate::audit::AuditEvent::SshConnectSuccess {
            server_id: server_id.clone(),
            session_id: session_id.clone(),
        },
    );

    // Update last_connected
    let now = time::OffsetDateTime::now_utc().to_string();
    let _ = state.db.with_conn(|conn| {
        conn.execute(
            "UPDATE servers SET last_connected = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, server_id],
        )
    });

    Ok(session_id)
}

/// Opens a shell channel on an already-authenticated SSH session.
/// Called by the frontend after the terminal UI is mounted and actual dimensions are known.
#[tauri::command]
pub async fn ssh_open_shell(
    state: State<'_, AppState>,
    app: AppHandle,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| SshError::SessionNotFound(session_id.clone()).to_string())?;

    // Check for exit proxy before opening shell
    let exit_proxy_url = session.exit_proxy_url.clone();

    session
        .open_shell(app.clone(), session_id.clone(), cols, rows)
        .await
        .map_err(|e| e.to_string())?;

    // Inject exit proxy env vars silently if post-target routing is active
    if let Some(url) = exit_proxy_url {
        let proxy_env = format!(
            " export ALL_PROXY={url} HTTP_PROXY={url} HTTPS_PROXY={url} http_proxy={url} https_proxy={url}\n"
        );
        let _ = session.write(proxy_env.as_bytes());
    }

    // Emit connected status now that shell is ready
    let status_event = format!("ssh://status/{session_id}");
    let _ = app.emit(
        &status_event,
        serde_json::json!({"status": "connected", "message": "shell opened"}),
    );

    Ok(())
}

/// Tests SSH connectivity using form input (without saving).
/// Uses the chain engine for proper session lifecycle management.
/// Accepts an optional `chain` parameter with the exact hop order from the UI.
#[tauri::command]
pub async fn ssh_test(
    state: State<'_, AppState>,
    app: AppHandle,
    host: String,
    port: u32,
    username: String,
    auth_type: String,
    password: Option<String>,
    key_path: Option<String>,
    passphrase: Option<String>,
    proxy_id: Option<String>,
    network_proxy_id: Option<String>,
    chain: Option<Vec<crate::storage::models::ChainHopInput>>,
) -> Result<String, String> {
    // Build pre-target and post-target hops from chain parameter
    let (pre_hops, post_hops) = if let Some(ref chain_hops) = chain {
        let mut pre = Vec::new();
        let mut post = Vec::new();
        for h in chain_hops {
            let resolved = resolve_single_hop(
                &state,
                &ChainHop {
                    id: String::new(),
                    server_id: String::new(),
                    position: 0,
                    hop_type: h.hop_type.clone(),
                    hop_id: h.hop_id.clone(),
                    phase: h.phase.clone(),
                    created_at: String::new(),
                },
            )
            .map_err(|e| e.to_string())?;
            if h.phase == "post" {
                post.push(resolved);
            } else {
                pre.push(resolved);
            }
        }
        (pre, post)
    } else {
        // Legacy fallback (no post-hops)
        let proxy_id = proxy_id.filter(|s| !s.is_empty());
        let network_proxy_id = network_proxy_id.filter(|s| !s.is_empty());
        let mut hops = Vec::new();

        if let Some(ref np_id) = network_proxy_id {
            let hop = resolve_single_hop(
                &state,
                &ChainHop {
                    id: String::new(),
                    server_id: String::new(),
                    position: 0,
                    hop_type: "proxy".into(),
                    hop_id: np_id.clone(),
                    phase: "pre".into(),
                    created_at: String::new(),
                },
            )
            .map_err(|e| e.to_string())?;
            hops.push(hop);
        }

        if let Some(ref b_id) = proxy_id {
            let hop = resolve_single_hop(
                &state,
                &ChainHop {
                    id: String::new(),
                    server_id: String::new(),
                    position: 0,
                    hop_type: "ssh".into(),
                    hop_id: b_id.clone(),
                    phase: "pre".into(),
                    created_at: String::new(),
                },
            )
            .map_err(|e| e.to_string())?;
            hops.push(hop);
        }

        (hops, Vec::new())
    };

    // Build target with form credentials (not from DB/keychain)
    let resolved_target = chain_connect::ResolvedTarget {
        server_id: String::new(),
        host: host.clone(),
        port: port as u16,
        username: username.clone(),
        auth_type: auth_type.clone(),
        password,
        key_path,
        passphrase,
    };

    // Connect through the chain (post-hops passed for index calculation only)
    let status_event = "ssh://test/status";
    let post_hops_count = post_hops.len();
    let result = chain_connect::connect_chain(
        state.inner(),
        &app,
        status_event,
        pre_hops,
        resolved_target,
        Vec::new(), // post_hops not passed to connect_chain (no exit routing in test)
    )
    .await
    .map_err(|e| e.to_string())?;

    // Test post-target hops connectivity (exit routing nodes)
    if !post_hops.is_empty() {
        let hop_offset = result.proxy_chain_ids.len() + 2; // Client(1) + pre_hops + target
        let test_result = chain_connect::test_post_hops(
            state.inner(),
            &app,
            status_event,
            &result.target_session,
            &post_hops,
            hop_offset,
        )
        .await;
        if let Err(e) = test_result {
            // Post-hop test failed — disconnect and report
            let _ = result.target_session.disconnect().await;
            return Err(e.to_string());
        }
    }

    // Disconnect target + cleanup bastion ref counts
    let proxy_chain = result.target_session.disconnect().await.map_err(|e| e.to_string())?;

    if !proxy_chain.is_empty() {
        let mut proxy_sessions = state.proxy_sessions.write().await;
        for bastion_id in proxy_chain {
            if let Some(entry) = proxy_sessions.get_mut(&bastion_id) {
                entry.ref_count = entry.ref_count.saturating_sub(1);
                if entry.ref_count == 0 {
                    if let Some(removed) = proxy_sessions.remove(&bastion_id) {
                        let _ = removed.session.disconnect().await;
                    }
                }
            }
        }
    }

    Ok("ok".into())
}

/// Disconnects an SSH session and cleans up proxy session references.
#[tauri::command]
pub async fn ssh_disconnect(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    // Audit: record disconnect
    crate::audit::log(
        &state.db,
        crate::audit::AuditEvent::SshDisconnect {
            session_id: session_id.clone(),
        },
    );

    // Also close SFTP session if open
    {
        let mut sftp_sessions = state.sftp_sessions.write().await;
        if let Some(sftp) = sftp_sessions.remove(&session_id) {
            // Try to unwrap the Arc; if there are pending transfers, just drop it
            if let Ok(sftp_handle) = std::sync::Arc::try_unwrap(sftp) {
                let _ = sftp_handle.close().await;
            }
        }
    }

    let session = {
        let mut sessions = state.sessions.write().await;
        sessions
            .remove(&session_id)
            .ok_or_else(|| SshError::SessionNotFound(session_id.clone()).to_string())?
    };

    // Disconnect and get proxy_chain for reference count cleanup
    let proxy_chain = session.disconnect().await.map_err(|e| e.to_string())?;

    // Decrement reference counts for all proxy sessions in the chain
    if !proxy_chain.is_empty() {
        let mut proxy_sessions = state.proxy_sessions.write().await;
        for bastion_id in proxy_chain {
            if let Some(entry) = proxy_sessions.get_mut(&bastion_id) {
                entry.ref_count = entry.ref_count.saturating_sub(1);
                eprintln!(">>> [PROXY] Decremented ref_count for {}: {}", bastion_id, entry.ref_count);

                // Close bastion connection if ref_count reaches 0
                if entry.ref_count == 0 {
                    proxy_sessions.remove(&bastion_id);
                    eprintln!(">>> [PROXY] Removed bastion connection: {} (ref_count = 0)", bastion_id);
                }
            }
        }
    }

    Ok(())
}

/// Writes user input data to the SSH shell channel. Non-blocking.
#[tauri::command]
pub async fn ssh_write(
    state: State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SshError::SessionNotFound(session_id).to_string())?;
    session.write(&data).map_err(|e| e.to_string())
}

/// Resizes the terminal window for an SSH session. Non-blocking.
#[tauri::command]
pub async fn ssh_resize(
    state: State<'_, AppState>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SshError::SessionNotFound(session_id).to_string())?;
    session.resize(cols, rows).map_err(|e| e.to_string())
}

/// Executes a command on the remote server via a separate exec channel.
/// Does not interfere with the PTY shell. Returns { stdout, exitCode }.
#[tauri::command]
pub async fn ssh_exec(
    state: State<'_, AppState>,
    session_id: String,
    command: String,
) -> Result<serde_json::Value, String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| SshError::SessionNotFound(session_id).to_string())?;
    let (stdout, exit_code) = session.exec_command(&command).await.map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "stdout": stdout.trim_end(),
        "exitCode": exit_code,
    }))
}

/// Responds to a host key change confirmation from the frontend.
/// Called by the frontend when the user accepts or rejects a changed host key.
#[tauri::command]
pub async fn ssh_host_key_respond(
    state: State<'_, AppState>,
    session_id: String,
    accepted: bool,
) -> Result<(), String> {
    let mut pending = state.pending_host_key_decisions.write().await;
    if let Some(tx) = pending.remove(&session_id) {
        let _ = tx.send(accepted);
    }
    Ok(())
}

/// Lists all known host keys from the database.
#[tauri::command]
pub fn ssh_known_hosts_list(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT host, port, key_type, fingerprint, first_seen, last_seen FROM known_hosts ORDER BY last_seen DESC",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "host": row.get::<_, String>(0)?,
                        "port": row.get::<_, i32>(1)?,
                        "keyType": row.get::<_, String>(2)?,
                        "fingerprint": row.get::<_, String>(3)?,
                        "firstSeen": row.get::<_, String>(4)?,
                        "lastSeen": row.get::<_, String>(5)?,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
        .map_err(|e| e.to_string())
}

/// Removes a known host key entry.
#[tauri::command]
pub fn ssh_known_hosts_remove(
    state: State<'_, AppState>,
    host: String,
    port: u32,
) -> Result<(), String> {
    host_key::remove_host_key(&state.db, &host, port as u16)
}

// ── Internal ───────────────────────────────────────────────────

#[derive(Clone)]
struct ServerInfo {
    server_id: String,
    host: String,
    port: i32,
    username: String,
    auth_type: String,
    password_enc: Option<Vec<u8>>,
    key_path: Option<String>,
    passphrase_enc: Option<Vec<u8>>,
    proxy_id: Option<String>,
    network_proxy_id: Option<String>,
}

/// Loads bastion server info from database.
fn load_bastion_info(
    state: &State<'_, AppState>,
    bastion_id: &str,
    app: &AppHandle,
    status_event: &str,
) -> Result<ServerInfo, String> {
    state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT host, port, username, auth_type, password_enc, key_path, passphrase_enc
                 FROM servers WHERE id = ?1",
                rusqlite::params![bastion_id],
                |row| {
                    Ok(ServerInfo {
                        host: row.get(0)?,
                        port: row.get(1)?,
                        username: row.get(2)?,
                        auth_type: row.get(3)?,
                        password_enc: row.get(4)?,
                        key_path: row.get(5)?,
                        passphrase_enc: row.get(6)?,
                        proxy_id: None,
                        network_proxy_id: None,
                        server_id: bastion_id.to_string(),
                    })
                },
            )
        })
        .map_err(|e| {
            let err = SshError::ServerNotFound(format!("Failed to load bastion server: {}", e));
            emit_error(app, status_event, &err)
        })
}

/// Authenticates an SSH session using server info credentials.
async fn auth_server(
    state: &State<'_, AppState>,
    app: &AppHandle,
    status_event: &str,
    session: &mut SshSession,
    info: &ServerInfo,
    label: &str,
) -> Result<(), String> {
    let auth_type = AuthType::from_str(&info.auth_type).unwrap_or(AuthType::Password);
    match auth_type {
        AuthType::Password => {
            let password = keychain::get(&keychain::ssh_password_key(&info.server_id))
                .unwrap_or_else(|_| decrypt_field(state, info.password_enc.clone()).unwrap_or_default());
            auth::auth_password(session.handle_mut(), &info.username, &password)
                .await
                .map_err(|e| emit_error(app, status_event, &SshError::AuthFailed(format!("{} auth failed: {}", label, e))))?;
        }
        AuthType::Key => {
            let key_path = info.key_path.as_deref()
                .ok_or_else(|| emit_error(app, status_event, &SshError::AuthFailed(format!("{}: no key path configured", label))))?;
            let passphrase = keychain::get(&keychain::ssh_passphrase_key(&info.server_id))
                .ok()
                .or_else(|| {
                    info.passphrase_enc.clone().and_then(|enc| {
                        decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                    })
                });
            auth::auth_key(session.handle_mut(), &info.username, key_path, passphrase.as_deref())
                .await
                .map_err(|e| emit_error(app, status_event, &SshError::AuthFailed(format!("{} auth failed: {}", label, e))))?;
        }
    }
    Ok(())
}

/// Emits an error status event and returns the error string.
fn emit_error(app: &AppHandle, event: &str, err: &SshError) -> String {
    let _ = app.emit(
        event,
        serde_json::json!({"status": "error", "message": err.to_string()}),
    );
    err.to_string()
}

/// Loads bastion server info from database (without event emission).
/// Used by `ssh_test` which has no active status event.
fn load_bastion_info_raw(
    state: &State<'_, AppState>,
    bastion_id: &str,
) -> Result<ServerInfo, String> {
    state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT host, port, username, auth_type, password_enc, key_path, passphrase_enc
                 FROM servers WHERE id = ?1",
                rusqlite::params![bastion_id],
                |row| {
                    Ok(ServerInfo {
                        host: row.get(0)?,
                        port: row.get(1)?,
                        username: row.get(2)?,
                        auth_type: row.get(3)?,
                        password_enc: row.get(4)?,
                        key_path: row.get(5)?,
                        passphrase_enc: row.get(6)?,
                        proxy_id: None,
                        network_proxy_id: None,
                        server_id: bastion_id.to_string(),
                    })
                },
            )
        })
        .map_err(|e| format!("Failed to load bastion server: {}", e))
}

/// Authenticates an SSH session using server info credentials (without event emission).
/// Used by `ssh_test` which has no active status event.
async fn auth_server_raw(
    state: &State<'_, AppState>,
    session: &mut SshSession,
    info: &ServerInfo,
) -> Result<(), String> {
    let auth_type = AuthType::from_str(&info.auth_type).unwrap_or(AuthType::Password);
    match auth_type {
        AuthType::Password => {
            let password = keychain::get(&keychain::ssh_password_key(&info.server_id))
                .unwrap_or_else(|_| decrypt_field(state, info.password_enc.clone()).unwrap_or_default());
            auth::auth_password(session.handle_mut(), &info.username, &password)
                .await
                .map_err(|e| format!("Bastion auth failed: {}", e))?;
        }
        AuthType::Key => {
            let key_path = info.key_path.as_deref()
                .ok_or_else(|| "Bastion: no key path configured".to_string())?;
            let passphrase = keychain::get(&keychain::ssh_passphrase_key(&info.server_id))
                .ok()
                .or_else(|| {
                    info.passphrase_enc.clone().and_then(|enc| {
                        decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                    })
                });
            auth::auth_key(session.handle_mut(), &info.username, key_path, passphrase.as_deref())
                .await
                .map_err(|e| format!("Bastion auth failed: {}", e))?;
        }
    }
    Ok(())
}

/// Decrypts an encrypted field using the master key (without event emission).
/// Used by `ssh_test` for resolving proxy passwords.
fn decrypt_field_raw(
    state: &State<'_, AppState>,
    encrypted: Option<Vec<u8>>,
) -> Result<String, String> {
    decrypt_field(state, encrypted)
}

/// Decrypts an encrypted field using the master key.
fn decrypt_field(
    state: &State<'_, AppState>,
    encrypted: Option<Vec<u8>>,
) -> Result<String, String> {
    let Some(data) = encrypted else {
        return Ok(String::new());
    };

    let mk = state.master_key.read().expect("master_key lock poisoned");
    if let Some(ref key) = *mk {
        let plaintext = aes::decrypt(key, &data).map_err(|e| e.to_string())?;
        String::from_utf8(plaintext).map_err(|e| e.to_string())
    } else {
        String::from_utf8(data).map_err(|e| e.to_string())
    }
}

// ── Chain resolution helpers ──────────────────────────────────

/// Resolves connection_chain DB rows into pre/post ResolvedHop vectors.
fn resolve_chain_hops(
    state: &State<'_, AppState>,
    chain_hops: &[ChainHop],
    app: &AppHandle,
    status_event: &str,
) -> Result<(Vec<ResolvedHop>, Vec<ResolvedHop>), String> {
    let mut pre_hops = Vec::new();
    let mut post_hops = Vec::new();

    for hop in chain_hops {
        let resolved = resolve_single_hop(state, hop)
            .map_err(|e| emit_error(app, status_event, &SshError::ProxyFailed(e)))?;

        if hop.phase == "post" {
            post_hops.push(resolved);
        } else {
            pre_hops.push(resolved);
        }
    }

    Ok((pre_hops, post_hops))
}

/// Resolves a single chain hop into a ResolvedHop with decrypted credentials.
fn resolve_single_hop(
    state: &State<'_, AppState>,
    hop: &ChainHop,
) -> Result<ResolvedHop, String> {
    match hop.hop_type.as_str() {
        "ssh" => {
            let info = load_bastion_info_raw(state, &hop.hop_id)?;
            let password = keychain::get(&keychain::ssh_password_key(&info.server_id))
                .ok()
                .or_else(|| {
                    info.password_enc.clone().and_then(|enc| {
                        decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                    })
                });
            let passphrase = keychain::get(&keychain::ssh_passphrase_key(&info.server_id))
                .ok()
                .or_else(|| {
                    info.passphrase_enc.clone().and_then(|enc| {
                        decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                    })
                });

            Ok(ResolvedHop::Ssh(SshHopInfo {
                server_id: info.server_id,
                host: info.host,
                port: info.port as u16,
                username: info.username,
                auth_type: info.auth_type,
                password,
                key_path: info.key_path,
                passphrase,
            }))
        }
        "proxy" => {
            let proxy_record = crate::storage::proxies::get(&state.db, &hop.hop_id)
                .map_err(|e| format!("Failed to load proxy: {}", e))?;
            let proxy_type = ProxyType::from_str(&proxy_record.proxy_type)
                .ok_or_else(|| format!("Unknown proxy type: {}", proxy_record.proxy_type))?;
            let proxy_password =
                keychain::get(&crate::commands::proxy::proxy_password_key(&hop.hop_id))
                    .ok()
                    .or_else(|| {
                        proxy_record.password_enc.and_then(|enc| {
                            decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                        })
                    });

            Ok(ResolvedHop::Proxy(ProxyHopInfo {
                proxy_id: hop.hop_id.clone(),
                name: proxy_record.name,
                config: ProxyConfig {
                    proxy_type,
                    host: proxy_record.host,
                    port: proxy_record.port as u16,
                    username: proxy_record.username,
                    password: proxy_password,
                    tls: ProxyTlsConfig {
                        enabled: proxy_record.tls_enabled,
                        verify: proxy_record.tls_verify,
                        ca_cert_path: proxy_record.ca_cert_path,
                        client_cert_path: proxy_record.client_cert_path,
                        client_key_path: proxy_record.client_key_path,
                    },
                    command: proxy_record.command,
                },
            }))
        }
        _ => Err(format!("Unknown hop type: {}", hop.hop_type)),
    }
}

/// Builds pre-target hops from legacy proxy_id / network_proxy_id fields.
fn build_legacy_pre_hops(
    state: &State<'_, AppState>,
    server: &ServerInfo,
    app: &AppHandle,
    status_event: &str,
) -> Result<Vec<ResolvedHop>, String> {
    let mut hops = Vec::new();

    let network_proxy_id = server.network_proxy_id.as_ref().filter(|s| !s.is_empty());
    let proxy_id = server.proxy_id.as_ref().filter(|s| !s.is_empty());

    // Network proxy first (if any)
    if let Some(np_id) = network_proxy_id {
        let proxy_record = crate::storage::proxies::get(&state.db, np_id)
            .map_err(|e| {
                emit_error(
                    app,
                    status_event,
                    &SshError::ProxyFailed(format!("Failed to load network proxy: {}", e)),
                )
            })?;
        let proxy_type = ProxyType::from_str(&proxy_record.proxy_type)
            .ok_or_else(|| {
                emit_error(
                    app,
                    status_event,
                    &SshError::ProxyFailed(format!("Unknown proxy type: {}", proxy_record.proxy_type)),
                )
            })?;
        let proxy_password =
            keychain::get(&crate::commands::proxy::proxy_password_key(np_id))
                .ok()
                .or_else(|| {
                    proxy_record.password_enc.and_then(|enc| {
                        decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                    })
                });

        hops.push(ResolvedHop::Proxy(ProxyHopInfo {
            proxy_id: np_id.clone(),
            name: proxy_record.name,
            config: ProxyConfig {
                proxy_type,
                host: proxy_record.host,
                port: proxy_record.port as u16,
                username: proxy_record.username,
                password: proxy_password,
                tls: ProxyTlsConfig {
                    enabled: proxy_record.tls_enabled,
                    verify: proxy_record.tls_verify,
                    ca_cert_path: proxy_record.ca_cert_path,
                    client_cert_path: proxy_record.client_cert_path,
                    client_key_path: proxy_record.client_key_path,
                },
                command: proxy_record.command,
            },
        }));
    }

    // SSH bastion after proxy (if any)
    if let Some(b_id) = proxy_id {
        let info = load_bastion_info(state, b_id, app, status_event)?;
        let password = keychain::get(&keychain::ssh_password_key(&info.server_id))
            .ok()
            .or_else(|| {
                info.password_enc.clone().and_then(|enc| {
                    decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                })
            });
        let passphrase = keychain::get(&keychain::ssh_passphrase_key(&info.server_id))
            .ok()
            .or_else(|| {
                info.passphrase_enc.clone().and_then(|enc| {
                    decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
                })
            });

        hops.push(ResolvedHop::Ssh(SshHopInfo {
            server_id: info.server_id,
            host: info.host,
            port: info.port as u16,
            username: info.username,
            auth_type: info.auth_type,
            password,
            key_path: info.key_path,
            passphrase,
        }));
    }

    Ok(hops)
}

/// Resolves target server credentials for the chain engine.
fn resolve_target_info(
    state: &State<'_, AppState>,
    server: &ServerInfo,
) -> Result<ResolvedTarget, String> {
    let password = keychain::get(&keychain::ssh_password_key(&server.server_id))
        .ok()
        .or_else(|| {
            server.password_enc.clone().and_then(|enc| {
                decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
            })
        });
    let passphrase = keychain::get(&keychain::ssh_passphrase_key(&server.server_id))
        .ok()
        .or_else(|| {
            server.passphrase_enc.clone().and_then(|enc| {
                decrypt_field(state, Some(enc)).ok().filter(|s| !s.is_empty())
            })
        });

    Ok(ResolvedTarget {
        server_id: server.server_id.clone(),
        host: server.host.clone(),
        port: server.port as u16,
        username: server.username.clone(),
        auth_type: server.auth_type.clone(),
        password,
        key_path: server.key_path.clone(),
        passphrase,
    })
}

// ── Legacy helpers (kept for ssh_test backward compat) ────────

/// Resolves a server's proxy chain by recursively querying proxy_id.
/// Returns the chain in connection order: [outermost_bastion, ..., intermediate_hop, target]
/// This allows us to connect outermost first, then tunnel through each hop to reach target.
///
/// Detects circular proxy configurations and returns an error if found.
#[allow(dead_code)]
fn resolve_proxy_chain(
    state: &State<'_, AppState>,
    server_id: &str,
) -> Result<Vec<ServerInfo>, String> {
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut current_id = server_id.to_string();

    // Recursively follow proxy_id chain backwards
    loop {
        if visited.contains(&current_id) {
            return Err(format!("Circular proxy configuration detected at server: {}", current_id));
        }
        visited.insert(current_id.clone());

        let server_info = state
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT id, host, port, username, auth_type, password_enc, key_path, passphrase_enc, proxy_id, network_proxy_id
                     FROM servers WHERE id = ?1",
                    rusqlite::params![current_id],
                    |row| {
                        Ok(ServerInfo {
                            server_id: row.get(0)?,
                            host: row.get(1)?,
                            port: row.get(2)?,
                            username: row.get(3)?,
                            auth_type: row.get(4)?,
                            password_enc: row.get(5)?,
                            key_path: row.get(6)?,
                            passphrase_enc: row.get(7)?,
                            proxy_id: row.get(8)?,
                            network_proxy_id: row.get(9)?,
                        })
                    },
                )
            })
            .map_err(|_| format!("Server not found: {}", current_id))?;

        chain.push(server_info.clone());

        // Move to proxy if it exists, otherwise we're done
        if let Some(proxy_id) = &server_info.proxy_id {
            current_id = proxy_id.clone();
        } else {
            break;
        }
    }

    // Reverse the chain so it goes from outermost_bastion → ... → intermediate → target
    // (we need outermost_bastion first to connect through chain)
    chain.reverse();

    Ok(chain)
}

/// Connects to a target server through a chain of bastion hosts.
/// For now, supports single bastion (Phase 2 will extend to multi-hop).
/// chain: [target] or [bastion, target]
#[allow(dead_code)]
async fn connect_via_chain(
    state: &State<'_, AppState>,
    app: &AppHandle,
    status_event: &str,
    chain: Vec<ServerInfo>,
) -> Result<(SshSession, Vec<String>), String> {
    if chain.is_empty() {
        return Err("Empty proxy chain".into());
    }

    let mut proxy_chain = Vec::new();
    let target = chain.last().unwrap();

    // Handle direct connection or via bastion(s)
    let mut ssh_session = if chain.len() == 1 {
        // Direct connection
        let _ = app.emit(
            status_event,
            serde_json::json!({"status": "connecting", "message": "connecting..."}),
        );
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            SshSession::connect(&target.host, target.port as u16),
        )
        .await
        .map_err(|_| {
            let err = SshError::ConnectionFailed("connection timed out (10s)".into());
            emit_error(&app, &status_event, &err)
        })?
        .map_err(|e| emit_error(&app, &status_event, &e))?
    } else {
        // Via bastion(s) - for now, only support one bastion (chain[0])
        let bastion = &chain[0];
        let _ = app.emit(
            status_event,
            serde_json::json!({"status": "connecting", "message": format!("connecting to bastion {}...", bastion.host)}),
        );

        // Check if bastion already in pool
        {
            let mut proxy_sessions = state.proxy_sessions.write().await;
            if proxy_sessions.contains_key(&bastion.server_id) {
                if let Some(entry) = proxy_sessions.get_mut(&bastion.server_id) {
                    entry.ref_count += 1;
                    eprintln!(">>> [PROXY] Reusing bastion connection: {} (ref_count: {})", bastion.server_id, entry.ref_count);
                }
                proxy_chain.push(bastion.server_id.clone());
            } else {
                // Connect to new bastion
                let mut bastion_session = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    SshSession::connect(&bastion.host, bastion.port as u16),
                )
                .await
                .map_err(|_| {
                    let err = SshError::ConnectionFailed("bastion connection timed out (10s)".into());
                    emit_error(&app, &status_event, &err)
                })?
                .map_err(|e| emit_error(&app, &status_event, &e))?;

                // Authenticate bastion
                let bastion_auth_type = AuthType::from_str(&bastion.auth_type).unwrap_or(AuthType::Password);
                match bastion_auth_type {
                    AuthType::Password => {
                        let password = keychain::get(&keychain::ssh_password_key(&bastion.server_id))
                            .unwrap_or_else(|_| decrypt_field(&state, bastion.password_enc.clone()).unwrap_or_default());
                        auth::auth_password(bastion_session.handle_mut(), &bastion.username, &password)
                            .await
                            .map_err(|e| emit_error(&app, &status_event, &SshError::AuthFailed(format!("Bastion auth failed: {}", e))))?;
                    }
                    AuthType::Key => {
                        let key_path = bastion.key_path.as_deref()
                            .ok_or_else(|| emit_error(&app, &status_event, &SshError::AuthFailed("bastion: no key path configured".into())))?;
                        let passphrase = keychain::get(&keychain::ssh_passphrase_key(&bastion.server_id))
                            .ok()
                            .or_else(|| {
                                bastion.passphrase_enc.clone().and_then(|enc| {
                                    decrypt_field(&state, Some(enc)).ok().filter(|s| !s.is_empty())
                                })
                            });
                        auth::auth_key(bastion_session.handle_mut(), &bastion.username, key_path, passphrase.as_deref())
                            .await
                            .map_err(|e| emit_error(&app, &status_event, &SshError::AuthFailed(format!("Bastion auth failed: {}", e))))?;
                    }
                }

                proxy_sessions.insert(bastion.server_id.clone(), crate::state::ProxyEntry {
                    session: Box::new(bastion_session),
                    ref_count: 1,
                });
                proxy_chain.push(bastion.server_id.clone());
            }
        }

        // Connect to target via bastion
        let _ = app.emit(
            status_event,
            serde_json::json!({"status": "connecting", "message": format!("connecting via bastion to target {}...", target.host)}),
        );

        let proxy_sessions = state.proxy_sessions.read().await;
        let bastion_entry = proxy_sessions.get(&bastion.server_id)
            .ok_or_else(|| {
                let err = SshError::ConnectionFailed("bastion session not found in pool".into());
                emit_error(&app, &status_event, &err)
            })?;
        let bastion_handle = bastion_entry.session.handle();

        let target_session = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            SshSession::connect_via_proxy(&bastion_handle, &target.host, target.port as u16),
        )
        .await
        .map_err(|_| {
            let err = SshError::ConnectionFailed("target connection timed out (10s)".into());
            emit_error(&app, &status_event, &err)
        })?
        .map_err(|e| emit_error(&app, &status_event, &e))?;

        drop(proxy_sessions);
        target_session
    };

    // Authenticate target
    let auth_type = AuthType::from_str(&target.auth_type).unwrap_or(AuthType::Password);
    match auth_type {
        AuthType::Password => {
            let password = keychain::get(&keychain::ssh_password_key(&target.server_id))
                .unwrap_or_else(|_| decrypt_field(&state, target.password_enc.clone()).unwrap_or_default());
            auth::auth_password(ssh_session.handle_mut(), &target.username, &password)
                .await
                .map_err(|e| emit_error(&app, &status_event, &e))?;
        }
        AuthType::Key => {
            let key_path = target.key_path.as_deref()
                .ok_or("no key path configured")?;
            let passphrase = keychain::get(&keychain::ssh_passphrase_key(&target.server_id))
                .ok()
                .or_else(|| {
                    target.passphrase_enc.clone().and_then(|enc| {
                        decrypt_field(&state, Some(enc)).ok().filter(|s| !s.is_empty())
                    })
                });
            auth::auth_key(ssh_session.handle_mut(), &target.username, key_path, passphrase.as_deref())
                .await
                .map_err(|e| emit_error(&app, &status_event, &e))?;
        }
    }

    ssh_session.proxy_chain = proxy_chain.clone();
    Ok((ssh_session, proxy_chain))
}
