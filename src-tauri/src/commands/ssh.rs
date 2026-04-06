use tauri::{AppHandle, Emitter, State};

use crate::crypto::aes;
use crate::keychain;
use crate::ssh::proxy::{ProxyConfig, ProxyTlsConfig, ProxyType};
use crate::ssh::session::SshSession;
use crate::ssh::{auth, SshError};
use crate::state::AppState;
use crate::storage::models::AuthType;

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

    // Load server details from database (including proxy_id and network_proxy_id)
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

    // Normalize empty strings to None for proxy fields
    let proxy_id = server.proxy_id.filter(|s| !s.is_empty());
    let network_proxy_id = server.network_proxy_id.filter(|s| !s.is_empty());

    // Resolve network proxy config if configured
    let network_proxy = if let Some(ref np_id) = network_proxy_id {
        let proxy_record = crate::storage::proxies::get(&state.db, np_id)
            .map_err(|e| {
                let err = SshError::ProxyFailed(format!("Failed to load network proxy: {}", e));
                emit_error(&app, &status_event, &err)
            })?;
        let proxy_type = ProxyType::from_str(&proxy_record.proxy_type)
            .ok_or_else(|| {
                let err = SshError::ProxyFailed(format!("Unknown proxy type: {}", proxy_record.proxy_type));
                emit_error(&app, &status_event, &err)
            })?;
        // Resolve proxy password
        let proxy_password = keychain::get(&crate::commands::proxy::proxy_password_key(np_id))
            .ok()
            .or_else(|| {
                proxy_record.password_enc.and_then(|enc| {
                    decrypt_field(&state, Some(enc)).ok().filter(|s| !s.is_empty())
                })
            });
        Some(ProxyConfig {
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
        })
    } else {
        None
    };

    // 4-branch connection logic: (network_proxy, bastion)
    let mut ssh_session;
    let mut proxy_chain = Vec::new();

    match (&network_proxy, &proxy_id) {
        // Branch 1: Network proxy + bastion
        (Some(np), Some(bastion_id)) => {
            let _ = app.emit(&status_event, serde_json::json!({"status": "connecting", "message": "connecting via proxy to bastion..."}));

            let bastion_info = load_bastion_info(&state, bastion_id, &app, &status_event)?;

            {
                let mut proxy_sessions = state.proxy_sessions.write().await;
                if proxy_sessions.contains_key(bastion_id) {
                    if let Some(entry) = proxy_sessions.get_mut(bastion_id) {
                        entry.ref_count += 1;
                    }
                } else {
                    let mut bastion_session = tokio::time::timeout(
                        std::time::Duration::from_secs(15),
                        SshSession::connect_via_network_proxy(np, &bastion_info.host, bastion_info.port as u16),
                    )
                    .await
                    .map_err(|_| emit_error(&app, &status_event, &SshError::ConnectionFailed("proxy→bastion timed out (15s)".into())))?
                    .map_err(|e| emit_error(&app, &status_event, &e))?;

                    auth_server(&state, &app, &status_event, &mut bastion_session, &bastion_info, "Bastion").await?;

                    proxy_sessions.insert(bastion_id.clone(), crate::state::ProxyEntry {
                        session: Box::new(bastion_session),
                        ref_count: 1,
                    });
                }
            }

            let _ = app.emit(&status_event, serde_json::json!({"status": "connecting", "message": "connecting via bastion to target..."}));

            let proxy_sessions = state.proxy_sessions.read().await;
            let bastion_entry = proxy_sessions.get(bastion_id)
                .ok_or_else(|| emit_error(&app, &status_event, &SshError::ConnectionFailed("bastion session not found in pool".into())))?;

            ssh_session = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect_via_proxy(bastion_entry.session.handle(), &server.host, server.port as u16),
            )
            .await
            .map_err(|_| emit_error(&app, &status_event, &SshError::ConnectionFailed("target via bastion timed out (10s)".into())))?
            .map_err(|e| emit_error(&app, &status_event, &e))?;

            drop(proxy_sessions);
            proxy_chain.push(bastion_id.clone());
        }

        // Branch 2: Network proxy only (no bastion)
        (Some(np), None) => {
            let _ = app.emit(&status_event, serde_json::json!({"status": "connecting", "message": "connecting via proxy..."}));

            ssh_session = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                SshSession::connect_via_network_proxy(np, &server.host, server.port as u16),
            )
            .await
            .map_err(|_| emit_error(&app, &status_event, &SshError::ConnectionFailed("proxy connection timed out (15s)".into())))?
            .map_err(|e| emit_error(&app, &status_event, &e))?;
        }

        // Branch 3: Bastion only (existing ProxyJump, no network proxy)
        (None, Some(bastion_id)) => {
            let _ = app.emit(&status_event, serde_json::json!({"status": "connecting", "message": "connecting to bastion..."}));

            let bastion_info = load_bastion_info(&state, bastion_id, &app, &status_event)?;

            {
                let mut proxy_sessions = state.proxy_sessions.write().await;
                if proxy_sessions.contains_key(bastion_id) {
                    if let Some(entry) = proxy_sessions.get_mut(bastion_id) {
                        entry.ref_count += 1;
                    }
                } else {
                    let mut bastion_session = tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        SshSession::connect(&bastion_info.host, bastion_info.port as u16),
                    )
                    .await
                    .map_err(|_| emit_error(&app, &status_event, &SshError::ConnectionFailed("bastion connection timed out (10s)".into())))?
                    .map_err(|e| emit_error(&app, &status_event, &e))?;

                    auth_server(&state, &app, &status_event, &mut bastion_session, &bastion_info, "Bastion").await?;

                    proxy_sessions.insert(bastion_id.clone(), crate::state::ProxyEntry {
                        session: Box::new(bastion_session),
                        ref_count: 1,
                    });
                }
            }

            let _ = app.emit(&status_event, serde_json::json!({"status": "connecting", "message": "connecting via bastion to target..."}));

            let proxy_sessions = state.proxy_sessions.read().await;
            let bastion_entry = proxy_sessions.get(bastion_id)
                .ok_or_else(|| emit_error(&app, &status_event, &SshError::ConnectionFailed("bastion session not found in pool".into())))?;

            ssh_session = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect_via_proxy(bastion_entry.session.handle(), &server.host, server.port as u16),
            )
            .await
            .map_err(|_| emit_error(&app, &status_event, &SshError::ConnectionFailed("target via bastion timed out (10s)".into())))?
            .map_err(|e| emit_error(&app, &status_event, &e))?;

            drop(proxy_sessions);
            proxy_chain.push(bastion_id.clone());
        }

        // Branch 4: Direct connection (no proxy, no bastion)
        (None, None) => {
            ssh_session = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect(&server.host, server.port as u16),
            )
            .await
            .map_err(|_| emit_error(&app, &status_event, &SshError::ConnectionFailed("connection timed out (10s)".into())))?
            .map_err(|e| emit_error(&app, &status_event, &e))?;
        }
    }

    // Authenticate target server
    let auth_type = AuthType::from_str(&server.auth_type).unwrap_or(AuthType::Password);
    match auth_type {
        AuthType::Password => {
            // Try keychain first, then legacy encrypted field
            let password = keychain::get(&keychain::ssh_password_key(&server.server_id))
                .unwrap_or_else(|_| decrypt_field(&state, server.password_enc).unwrap_or_default());
            auth::auth_password(ssh_session.handle_mut(), &server.username, &password)
                .await
                .map_err(|e| emit_error(&app, &status_event, &e))?;
        }
        AuthType::Key => {
            let key_path = server
                .key_path
                .as_deref()
                .ok_or("no key path configured")?;
            // Try keychain first for passphrase
            let passphrase = keychain::get(&keychain::ssh_passphrase_key(&server.server_id))
                .ok()
                .or_else(|| {
                    server.passphrase_enc.and_then(|enc| {
                        decrypt_field(&state, Some(enc)).ok().filter(|s| !s.is_empty())
                    })
                });
            auth::auth_key(
                ssh_session.handle_mut(),
                &server.username,
                key_path,
                passphrase.as_deref(),
            )
            .await
            .map_err(|e| emit_error(&app, &status_event, &e))?;
        }
    }

    // Store proxy_chain in session for later cleanup
    ssh_session.proxy_chain = proxy_chain;

    // Store session (shell not opened yet — frontend calls ssh_open_shell after terminal mount)
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id.clone(), ssh_session);
    }

    // Emit authenticated status (shell not yet open)
    let _ = app.emit(
        &status_event,
        serde_json::json!({
            "status": "authenticated",
            "message": format!("{}@{}:{}", server.username, server.host, server.port),
        }),
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

    session
        .open_shell(app.clone(), session_id.clone(), cols, rows)
        .await
        .map_err(|e| e.to_string())?;

    // Emit connected status now that shell is ready
    let status_event = format!("ssh://status/{session_id}");
    let _ = app.emit(
        &status_event,
        serde_json::json!({"status": "connected", "message": "shell opened"}),
    );

    Ok(())
}

/// Tests SSH connectivity using form input (without saving).
/// Supports bastion and network proxy to ensure the test path matches the real connection path.
#[tauri::command]
pub async fn ssh_test(
    state: State<'_, AppState>,
    host: String,
    port: u32,
    username: String,
    auth_type: String,
    password: Option<String>,
    key_path: Option<String>,
    passphrase: Option<String>,
    proxy_id: Option<String>,
    network_proxy_id: Option<String>,
) -> Result<String, String> {
    let proxy_id = proxy_id.filter(|s| !s.is_empty());
    let network_proxy_id = network_proxy_id.filter(|s| !s.is_empty());

    // Resolve network proxy config if configured
    let network_proxy = if let Some(ref np_id) = network_proxy_id {
        let proxy_record = crate::storage::proxies::get(&state.db, np_id)
            .map_err(|e| format!("Failed to load network proxy: {}", e))?;
        let proxy_type = ProxyType::from_str(&proxy_record.proxy_type)
            .ok_or_else(|| format!("Unknown proxy type: {}", proxy_record.proxy_type))?;
        let proxy_password = keychain::get(&crate::commands::proxy::proxy_password_key(np_id))
            .ok()
            .or_else(|| {
                proxy_record.password_enc.and_then(|enc| {
                    decrypt_field_raw(&state, Some(enc)).ok().filter(|s| !s.is_empty())
                })
            });
        Some(ProxyConfig {
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
        })
    } else {
        None
    };

    // Build SSH session through the same path as ssh_connect
    let mut ssh_session = match (&network_proxy, &proxy_id) {
        // Network proxy + bastion
        (Some(np), Some(bastion_id)) => {
            let bastion_info = load_bastion_info_raw(&state, bastion_id)?;
            let mut bastion_session = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                SshSession::connect_via_network_proxy(np, &bastion_info.host, bastion_info.port as u16),
            )
            .await
            .map_err(|_| "proxy→bastion timed out (15s)".to_string())?
            .map_err(|e| e.to_string())?;

            auth_server_raw(&state, &mut bastion_session, &bastion_info).await?;

            let target_session = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect_via_proxy(bastion_session.handle(), &host, port as u16),
            )
            .await
            .map_err(|_| "bastion→target timed out (10s)".to_string())?
            .map_err(|e| e.to_string())?;

            // Disconnect bastion after test
            let _ = bastion_session.disconnect().await;
            target_session
        }
        // Network proxy only
        (Some(np), None) => {
            tokio::time::timeout(
                std::time::Duration::from_secs(15),
                SshSession::connect_via_network_proxy(np, &host, port as u16),
            )
            .await
            .map_err(|_| "proxy connection timed out (15s)".to_string())?
            .map_err(|e| e.to_string())?
        }
        // Bastion only
        (None, Some(bastion_id)) => {
            let bastion_info = load_bastion_info_raw(&state, bastion_id)?;
            let mut bastion_session = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect(&bastion_info.host, bastion_info.port as u16),
            )
            .await
            .map_err(|_| "bastion connection timed out (10s)".to_string())?
            .map_err(|e| e.to_string())?;

            auth_server_raw(&state, &mut bastion_session, &bastion_info).await?;

            let target_session = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect_via_proxy(bastion_session.handle(), &host, port as u16),
            )
            .await
            .map_err(|_| "bastion→target timed out (10s)".to_string())?
            .map_err(|e| e.to_string())?;

            let _ = bastion_session.disconnect().await;
            target_session
        }
        // Direct connection
        (None, None) => {
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect(&host, port as u16),
            )
            .await
            .map_err(|_| "connection timed out (10s)".to_string())?
            .map_err(|e| e.to_string())?
        }
    };

    // Authenticate target
    let at = AuthType::from_str(&auth_type).unwrap_or(AuthType::Password);
    match at {
        AuthType::Password => {
            let pw = password.unwrap_or_default();
            auth::auth_password(ssh_session.handle_mut(), &username, &pw)
                .await
                .map_err(|e| e.to_string())?;
        }
        AuthType::Key => {
            let kp = key_path.as_deref().ok_or("no key path")?;
            auth::auth_key(ssh_session.handle_mut(), &username, kp, passphrase.as_deref())
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    // Disconnect immediately
    let _ = ssh_session.disconnect().await;

    Ok("ok".into())
}

/// Disconnects an SSH session and cleans up proxy session references.
#[tauri::command]
pub async fn ssh_disconnect(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
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

/// Resolves a server's proxy chain by recursively querying proxy_id.
/// Returns the chain in connection order: [outermost_bastion, ..., intermediate_hop, target]
/// This allows us to connect outermost first, then tunnel through each hop to reach target.
///
/// Detects circular proxy configurations and returns an error if found.
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
