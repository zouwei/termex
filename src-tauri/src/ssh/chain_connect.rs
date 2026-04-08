//! Multi-hop connection chain engine.
//!
//! Walks an ordered list of pre-target hops (SSH bastions and network proxies) to build
//! a tunnel reaching the target, then optionally sets up post-target hops for exit routing.

use russh::client;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use super::auth;
use super::proxy::{self, AsyncStream, ProxyConfig};
use super::session::SshSession;
use super::SshError;
use crate::state::{AppState, ProxyEntry};

// ── Types ──────────────────────────────────────────────────────

/// A resolved hop ready for connection (credentials already decrypted).
#[derive(Debug, Clone)]
pub enum ResolvedHop {
    /// An SSH bastion server.
    Ssh(SshHopInfo),
    /// A network proxy (SOCKS5/SOCKS4/HTTP/Tor/ProxyCommand).
    Proxy(ProxyHopInfo),
}

/// SSH hop information with decrypted credentials.
#[derive(Debug, Clone)]
pub struct SshHopInfo {
    pub server_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub passphrase: Option<String>,
}

/// Network proxy hop information.
#[derive(Debug, Clone)]
pub struct ProxyHopInfo {
    pub proxy_id: String,
    pub name: String,
    pub config: ProxyConfig,
}

/// Target server information with decrypted credentials.
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub server_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub passphrase: Option<String>,
}

/// Result of chain connection.
pub struct ChainResult {
    pub target_session: SshSession,
    /// All SSH bastion server_ids in the chain (for ref-count cleanup).
    pub proxy_chain_ids: Vec<String>,
    /// Post-target hops to set up after session is stored (exit routing).
    pub post_hops: Vec<ResolvedHop>,
}

// ── Intermediate state during chain walk ────────────────────────

/// The "current position" in the chain after processing each hop.
enum ChainState {
    /// No hops processed yet — need to connect from local machine.
    Start,
    /// Last hop was an SSH session stored in the proxy_sessions pool.
    /// We look up the handle by server_id when we need it.
    SshSession {
        server_id: String,
    },
    /// Last hop was a network proxy. We have a raw stream to the proxy server
    /// plus the proxy config needed to perform the CONNECT handshake.
    ProxyStream {
        stream: Box<dyn AsyncStream>,
        config: ProxyConfig,
        /// If this stream came from an SSH direct-tcpip channel, the bastion's server_id.
        /// Used to recreate the stream for SOCKS5 TLS auto-retry.
        ssh_source: Option<String>,
    },
}

// ── Main entry point ───────────────────────────────────────────

/// Connects through a chain of hops to reach the target server.
///
/// 1. Walks `pre_hops` to build the tunnel
/// 2. Connects to + authenticates the target
/// 3. Returns the session and post_hops for exit routing (set up later by caller)
pub async fn connect_chain(
    state: &AppState,
    app: &AppHandle,
    status_event: &str,
    pre_hops: Vec<ResolvedHop>,
    target: ResolvedTarget,
    post_hops: Vec<ResolvedHop>,
) -> Result<ChainResult, SshError> {
    let mut proxy_chain_ids = Vec::new();

    // ── Phase 1: Walk pre-target hops ──

    let mut chain_state = ChainState::Start;

    // hop_index 0 = Client (always OK), actual chain starts at index 1
    for (i, hop) in pre_hops.iter().enumerate() {
        let hop_label = match hop {
            ResolvedHop::Ssh(info) => format!("SSH {}", info.host),
            ResolvedHop::Proxy(info) => format!("Proxy {}", info.name),
        };
        let hop_index = i + 1; // +1 because Client is index 0
        let _ = app.emit(
            status_event,
            serde_json::json!({
                "status": "hop_connecting",
                "hopIndex": hop_index,
                "message": format!("hop {}/{}: {}...", i + 1, pre_hops.len(), hop_label),
            }),
        );

        match process_hop(state, hop, chain_state, &mut proxy_chain_ids).await {
            Ok(new_state) => {
                chain_state = new_state;
                let _ = app.emit(
                    status_event,
                    serde_json::json!({
                        "status": "hop_ok",
                        "hopIndex": hop_index,
                    }),
                );
            }
            Err(e) => {
                let _ = app.emit(
                    status_event,
                    serde_json::json!({
                        "status": "hop_failed",
                        "hopIndex": hop_index,
                        "message": e.to_string(),
                    }),
                );
                return Err(e);
            }
        }
    }

    // ── Phase 2: Connect to target through the final chain state ──

    // Target is the last node in the connection path
    let target_hop_index = pre_hops.len() + 1; // Client(0) + pre_hops + target

    let _ = app.emit(
        status_event,
        serde_json::json!({
            "status": "hop_connecting",
            "hopIndex": target_hop_index,
            "message": format!("connecting to target {}:{}...", target.host, target.port),
        }),
    );

    let mut target_session = match connect_target(state, &target, chain_state).await {
        Ok(s) => s,
        Err(e) => {
            let _ = app.emit(
                status_event,
                serde_json::json!({
                    "status": "hop_failed",
                    "hopIndex": target_hop_index,
                    "message": e.to_string(),
                }),
            );
            return Err(e);
        }
    };

    // Authenticate target
    match auth_session(&mut target_session, &target).await {
        Ok(()) => {
            let _ = app.emit(
                status_event,
                serde_json::json!({
                    "status": "hop_ok",
                    "hopIndex": target_hop_index,
                }),
            );
        }
        Err(e) => {
            let _ = app.emit(
                status_event,
                serde_json::json!({
                    "status": "hop_failed",
                    "hopIndex": target_hop_index,
                    "message": e.to_string(),
                }),
            );
            return Err(e);
        }
    }

    // Store chain info on the session
    target_session.proxy_chain = proxy_chain_ids.clone();

    Ok(ChainResult {
        target_session,
        proxy_chain_ids,
        post_hops,
    })
}

// ── Hop processing ─────────────────────────────────────────────

/// Processes a single hop in the chain, advancing the chain state.
async fn process_hop(
    state: &AppState,
    hop: &ResolvedHop,
    current: ChainState,
    proxy_chain_ids: &mut Vec<String>,
) -> Result<ChainState, SshError> {
    match (hop, current) {
        // ── SSH hop from start (direct TCP connection) ──
        (ResolvedHop::Ssh(info), ChainState::Start) => {
            connect_or_reuse_bastion_direct(state, info).await?;
            proxy_chain_ids.push(info.server_id.clone());
            Ok(ChainState::SshSession {
                server_id: info.server_id.clone(),
            })
        }

        // ── SSH hop after another SSH (direct-tcpip tunnel) ──
        (ResolvedHop::Ssh(info), ChainState::SshSession { server_id: prev_id }) => {
            connect_or_reuse_bastion_via_pool(state, info, &prev_id).await?;
            proxy_chain_ids.push(info.server_id.clone());
            Ok(ChainState::SshSession {
                server_id: info.server_id.clone(),
            })
        }

        // ── SSH hop after a proxy (proxy handshake to SSH, then SSH over stream) ──
        (ResolvedHop::Ssh(info), ChainState::ProxyStream { stream, config, .. }) => {
            // Perform proxy handshake to reach the SSH hop (with 10s timeout)
            let tunneled = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                proxy::connect_via_proxy_on_stream(&config, &info.host, info.port, stream),
            )
            .await
            .map_err(|_| SshError::ProxyFailed(
                format!("proxy handshake to {}:{} timed out (10s)", info.host, info.port)
            ))??;
            connect_bastion_on_stream(state, info, tunneled).await?;
            proxy_chain_ids.push(info.server_id.clone());
            Ok(ChainState::SshSession {
                server_id: info.server_id.clone(),
            })
        }

        // ── Proxy hop from start (direct TCP to proxy) ──
        (ResolvedHop::Proxy(info), ChainState::Start) => {
            // Create a TCP connection to the proxy server.
            // The actual proxy CONNECT handshake is deferred until the next hop/target,
            // because we need the next hop's address to do CONNECT.
            let stream = create_proxy_tcp_stream(&info.config).await?;
            Ok(ChainState::ProxyStream {
                stream,
                config: info.config.clone(),
                ssh_source: None,
            })
        }

        // ── Proxy hop after SSH (open direct-tcpip to proxy server) ──
        (ResolvedHop::Proxy(info), ChainState::SshSession { server_id: prev_id }) => {
            eprintln!(
                ">>> [CHAIN] Proxy after SSH: proxy={}:{} tls={} via bastion={}",
                info.config.host, info.config.port, info.config.tls.enabled, prev_id
            );
            // Open direct-tcpip channel from the SSH session to the proxy server
            let proxy_sessions = state.proxy_sessions.read().await;
            let entry = proxy_sessions.get(&prev_id).ok_or_else(|| {
                SshError::ConnectionFailed(format!("bastion session {} not found in pool", prev_id))
            })?;
            let channel = entry
                .session
                .handle()
                .channel_open_direct_tcpip(
                    &info.config.host,
                    info.config.port as u32,
                    "127.0.0.1",
                    0,
                )
                .await
                .map_err(|e| {
                    SshError::ProxyFailed(format!(
                        "Failed to tunnel to proxy {}:{}: {}",
                        info.config.host, info.config.port, e
                    ))
                })?;
            drop(proxy_sessions);
            let stream: Box<dyn AsyncStream> = Box::new(channel.into_stream());
            Ok(ChainState::ProxyStream {
                stream,
                config: info.config.clone(),
                ssh_source: Some(prev_id.clone()),
            })
        }

        // ── Proxy hop after another proxy (chain proxies) ──
        (ResolvedHop::Proxy(info), ChainState::ProxyStream { stream: prev_stream, config: prev_config, .. }) => {
            // Perform the PREVIOUS proxy's handshake to reach THIS proxy's host:port (with 10s timeout)
            let stream = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                proxy::connect_via_proxy_on_stream(
                    &prev_config, &info.config.host, info.config.port, prev_stream,
                ),
            )
            .await
            .map_err(|_| SshError::ProxyFailed(
                format!("proxy handshake to {}:{} timed out (10s)", info.config.host, info.config.port)
            ))??;
            // Now we have a raw stream to THIS proxy — store its config for deferred handshake
            Ok(ChainState::ProxyStream {
                stream,
                config: info.config.clone(),
                ssh_source: None, // came from another proxy, not SSH
            })
        }
    }
}

/// Recreates a proxy stream for TLS retry.
/// If the original stream came from an SSH channel (ssh_source is set), opens a new direct-tcpip.
/// Otherwise, creates a new TCP connection.
async fn recreate_proxy_stream(
    state: &AppState,
    config: &ProxyConfig,
    ssh_source: Option<&str>,
) -> Result<Box<dyn AsyncStream>, SshError> {
    if let Some(bastion_id) = ssh_source {
        // Re-open direct-tcpip channel through the SSH bastion
        let proxy_sessions = state.proxy_sessions.read().await;
        let entry = proxy_sessions.get(bastion_id).ok_or_else(|| {
            SshError::ConnectionFailed(format!("bastion {} not found in pool for TLS retry", bastion_id))
        })?;
        let channel = entry
            .session
            .handle()
            .channel_open_direct_tcpip(&config.host, config.port as u32, "127.0.0.1", 0)
            .await
            .map_err(|e| {
                SshError::ProxyFailed(format!(
                    "TLS retry: failed to tunnel to proxy {}:{}: {}",
                    config.host, config.port, e
                ))
            })?;
        drop(proxy_sessions);
        Ok(Box::new(channel.into_stream()))
    } else {
        // Direct TCP connection
        create_proxy_tcp_stream(config).await
    }
}

/// Creates a raw TCP stream to a proxy server (for first-hop proxy).
async fn create_proxy_tcp_stream(config: &ProxyConfig) -> Result<Box<dyn AsyncStream>, SshError> {
    let addr = format!("{}:{}", config.host, config.port);
    let stream = tokio::net::TcpStream::connect(&addr)
        .await
        .map_err(|e| SshError::ProxyFailed(format!("TCP connect to proxy {}: {}", addr, e)))?;
    Ok(Box::new(stream))
}

// ── Target connection ──────────────────────────────────────────

/// Connects to the target server using the current chain state.
async fn connect_target(
    app_state: &AppState,
    target: &ResolvedTarget,
    state: ChainState,
) -> Result<SshSession, SshError> {
    match state {
        ChainState::Start => {
            // Direct connection (no pre-hops)
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect(&target.host, target.port),
            )
            .await
            .map_err(|_| SshError::ConnectionFailed("connection timed out (10s)".into()))?
        }

        ChainState::SshSession { server_id } => {
            // Connect via direct-tcpip through the last SSH hop in the pool
            let proxy_sessions = app_state.proxy_sessions.read().await;
            let entry = proxy_sessions.get(&server_id).ok_or_else(|| {
                SshError::ConnectionFailed(format!("bastion {} not found in pool", server_id))
            })?;
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect_via_proxy(entry.session.handle(), &target.host, target.port),
            )
            .await
            .map_err(|_| SshError::ConnectionFailed("target via bastion timed out (10s)".into()))?;
            drop(proxy_sessions);
            result
        }

        ChainState::ProxyStream { stream, config, ssh_source } => {
            eprintln!(
                ">>> [CHAIN] Target via ProxyStream: proxy={}:{} tls={} ssh_source={:?}",
                config.host, config.port, config.tls.enabled, ssh_source
            );

            // Proxy handshake with 10s timeout (covers plain attempt + TLS retry).
            // Without this, a silently dropped connection (GFW) hangs read_exact forever.
            let proxy_handshake = async {
                match proxy::connect_via_proxy_on_stream(
                    &config, &target.host, target.port, stream,
                ).await {
                    Ok(s) => Ok(s),
                    Err(e) if !config.tls.enabled
                        && matches!(config.proxy_type, proxy::ProxyType::Socks5 | proxy::ProxyType::Tor)
                        && proxy::is_socks5_tls_retryable(&e) =>
                    {
                        eprintln!(">>> [CHAIN] SOCKS5 plain failed ({e}), retrying with TLS...");
                        let new_stream = recreate_proxy_stream(app_state, &config, ssh_source.as_deref()).await?;
                        let mut tls_config = config.clone();
                        tls_config.tls.enabled = true;
                        proxy::connect_via_proxy_on_stream(
                            &tls_config, &target.host, target.port, new_stream,
                        ).await.map_err(|_| e) // TLS also failed → return original error
                    }
                    Err(e) => Err(e),
                }
            };

            let tunneled = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                proxy_handshake,
            )
            .await
            .map_err(|_| SshError::ProxyFailed(
                format!("proxy handshake timed out (10s) — {} may be unreachable from the bastion", config.host)
            ))??;

            // SSH handshake over the proxy tunnel
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                SshSession::connect_on_stream(tunneled),
            )
            .await
            .map_err(|_| SshError::ConnectionFailed("target via proxy timed out (10s)".into()))?
        }
    }
}

/// Authenticates a session using the resolved credentials.
async fn auth_session(
    session: &mut SshSession,
    creds: &ResolvedTarget,
) -> Result<(), SshError> {
    match creds.auth_type.as_str() {
        "key" => {
            let key_path = creds
                .key_path
                .as_deref()
                .ok_or_else(|| SshError::AuthFailed("no key path configured".into()))?;
            auth::auth_key(
                session.handle_mut(),
                &creds.username,
                key_path,
                creds.passphrase.as_deref(),
            )
            .await
        }
        _ => {
            let password = creds.password.as_deref().unwrap_or("");
            auth::auth_password(session.handle_mut(), &creds.username, password).await
        }
    }
}

// ── Bastion session management ─────────────────────────────────

/// Connects to (or reuses) a bastion SSH session via direct TCP.
/// Ensures the session is stored in proxy_sessions pool.
async fn connect_or_reuse_bastion_direct(
    state: &AppState,
    info: &SshHopInfo,
) -> Result<(), SshError> {
    let mut proxy_sessions = state.proxy_sessions.write().await;

    if let Some(entry) = proxy_sessions.get_mut(&info.server_id) {
        entry.ref_count += 1;
        return Ok(());
    }

    let mut session = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        SshSession::connect(&info.host, info.port),
    )
    .await
    .map_err(|_| {
        SshError::ConnectionFailed(format!(
            "bastion {} connection timed out (10s)",
            info.host
        ))
    })?
    .map_err(|e| e)?;

    auth_bastion(&mut session, info).await?;

    proxy_sessions.insert(
        info.server_id.clone(),
        ProxyEntry {
            session: Box::new(session),
            ref_count: 1,
        },
    );

    Ok(())
}

/// Connects to (or reuses) a bastion via another SSH session from the pool.
async fn connect_or_reuse_bastion_via_pool(
    state: &AppState,
    info: &SshHopInfo,
    prev_server_id: &str,
) -> Result<(), SshError> {
    {
        let mut proxy_sessions = state.proxy_sessions.write().await;
        if let Some(entry) = proxy_sessions.get_mut(&info.server_id) {
            entry.ref_count += 1;
            return Ok(());
        }
    }

    // Need to read the previous session handle while not holding write lock
    let proxy_sessions = state.proxy_sessions.read().await;
    let prev_entry = proxy_sessions.get(prev_server_id).ok_or_else(|| {
        SshError::ConnectionFailed(format!("bastion {} not found in pool", prev_server_id))
    })?;

    let mut session = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        SshSession::connect_via_proxy(prev_entry.session.handle(), &info.host, info.port),
    )
    .await
    .map_err(|_| {
        SshError::ConnectionFailed(format!(
            "bastion {} via SSH timed out (10s)",
            info.host
        ))
    })?
    .map_err(|e| e)?;

    drop(proxy_sessions);

    auth_bastion(&mut session, info).await?;

    let mut proxy_sessions = state.proxy_sessions.write().await;
    proxy_sessions.insert(
        info.server_id.clone(),
        ProxyEntry {
            session: Box::new(session),
            ref_count: 1,
        },
    );

    Ok(())
}

/// Connects a bastion SSH session over an existing stream (e.g., proxy tunnel).
async fn connect_bastion_on_stream(
    state: &AppState,
    info: &SshHopInfo,
    stream: Box<dyn AsyncStream>,
) -> Result<(), SshError> {
    {
        let mut proxy_sessions = state.proxy_sessions.write().await;
        if let Some(entry) = proxy_sessions.get_mut(&info.server_id) {
            entry.ref_count += 1;
            return Ok(());
        }
    }

    let mut session = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        SshSession::connect_on_stream(stream),
    )
    .await
    .map_err(|_| {
        SshError::ConnectionFailed(format!(
            "bastion {} via proxy timed out (10s)",
            info.host
        ))
    })?
    .map_err(|e| e)?;

    auth_bastion(&mut session, info).await?;

    let mut proxy_sessions = state.proxy_sessions.write().await;
    proxy_sessions.insert(
        info.server_id.clone(),
        ProxyEntry {
            session: Box::new(session),
            ref_count: 1,
        },
    );

    Ok(())
}

/// Authenticates a bastion SSH session.
async fn auth_bastion(session: &mut SshSession, info: &SshHopInfo) -> Result<(), SshError> {
    match info.auth_type.as_str() {
        "key" => {
            let key_path = info
                .key_path
                .as_deref()
                .ok_or_else(|| {
                    SshError::AuthFailed(format!("bastion {}: no key path", info.host))
                })?;
            auth::auth_key(
                session.handle_mut(),
                &info.username,
                key_path,
                info.passphrase.as_deref(),
            )
            .await
            .map_err(|e| {
                SshError::AuthFailed(format!("bastion {} auth failed: {}", info.host, e))
            })
        }
        _ => {
            let password = info.password.as_deref().unwrap_or("");
            auth::auth_password(session.handle_mut(), &info.username, password)
                .await
                .map_err(|e| {
                    SshError::AuthFailed(format!("bastion {} auth failed: {}", info.host, e))
                })
        }
    }
}

// ── Utilities ──────────────────────────────────────────────────

/// Percent-encodes a string for use in a URL userinfo component.
/// Encodes all characters except unreserved ones (RFC 3986: A-Z a-z 0-9 - . _ ~).
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}

// ── Exit routing (post-target) ─────────────────────────────────

/// Builds a proxy URL string from a ProxyConfig (for non-TLS proxies).
fn build_proxy_url(config: &ProxyConfig) -> Result<String, SshError> {
    match config.proxy_type {
        super::proxy::ProxyType::Socks5 | super::proxy::ProxyType::Tor => {
            if let (Some(user), Some(pass)) = (&config.username, &config.password) {
                Ok(format!("socks5://{}:{}@{}:{}", url_encode(user), url_encode(pass), config.host, config.port))
            } else {
                Ok(format!("socks5://{}:{}", config.host, config.port))
            }
        }
        super::proxy::ProxyType::Socks4 => {
            Ok(format!("socks4://{}:{}", config.host, config.port))
        }
        super::proxy::ProxyType::Http => {
            if let (Some(user), Some(pass)) = (&config.username, &config.password) {
                Ok(format!("http://{}:{}@{}:{}", url_encode(user), url_encode(pass), config.host, config.port))
            } else {
                Ok(format!("http://{}:{}", config.host, config.port))
            }
        }
        _ => Err(SshError::ProxyFailed("ProxyCommand cannot be used as exit proxy".into())),
    }
}

/// Tests connectivity for post-target hops (exit routing nodes) without full setup.
///
/// Used by `ssh_test` to verify that each post-target node is reachable:
/// - SSH hop: open direct-tcpip from previous hop → authenticate → disconnect
/// - Proxy hop: open direct-tcpip from previous hop → TCP connectable
///
/// Emits hop_connecting/hop_ok/hop_failed events for the traffic light UI.
pub async fn test_post_hops(
    state: &AppState,
    app: &AppHandle,
    status_event: &str,
    target_session: &SshSession,
    post_hops: &[ResolvedHop],
    hop_offset: usize, // index of the first post-hop in the connection path
) -> Result<(), SshError> {
    // Track the "current handle" for chaining: start from target
    // For SSH hops, we connect + auth and can chain further.
    // For Proxy hops, we just verify TCP reachability.
    let mut current_handle: Option<&client::Handle<auth::ClientHandler>> = Some(target_session.handle());
    let mut temp_sessions: Vec<SshSession> = Vec::new();

    for (i, hop) in post_hops.iter().enumerate() {
        let hop_index = hop_offset + i;

        let _ = app.emit(
            status_event,
            serde_json::json!({ "status": "hop_connecting", "hopIndex": hop_index }),
        );

        let result = match hop {
            ResolvedHop::Ssh(info) => {
                // Test SSH connectivity: direct-tcpip → SSH handshake → authenticate
                let handle = current_handle.ok_or_else(|| {
                    SshError::ProxyFailed("No SSH session to chain from".into())
                })?;

                match tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    let mut session = SshSession::connect_via_proxy(handle, &info.host, info.port).await?;
                    auth_hop(&mut session, info).await?;
                    Ok::<SshSession, SshError>(session)
                }).await {
                    Ok(Ok(session)) => {
                        temp_sessions.push(session);
                        // Update current_handle to the newly connected session
                        current_handle = Some(temp_sessions.last().unwrap().handle());
                        Ok(())
                    }
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(SshError::ConnectionFailed(
                        format!("SSH hop {}:{} timed out (10s)", info.host, info.port),
                    )),
                }
            }
            ResolvedHop::Proxy(info) => {
                // Test proxy reachability: open direct-tcpip to proxy host:port
                let handle = current_handle.ok_or_else(|| {
                    SshError::ProxyFailed("No SSH session to test proxy from".into())
                })?;

                match tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    let channel = handle
                        .channel_open_direct_tcpip(&info.config.host, info.config.port as u32, "127.0.0.1", 0)
                        .await
                        .map_err(|e| SshError::ProxyFailed(format!(
                            "Cannot reach proxy {}:{}: {}", info.config.host, info.config.port, e
                        )))?;
                    // TCP connected — close the channel
                    drop(channel);
                    Ok::<(), SshError>(())
                }).await {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(SshError::ProxyFailed(
                        format!("Proxy {}:{} unreachable (10s timeout)", info.config.host, info.config.port),
                    )),
                }
            }
        };

        match result {
            Ok(()) => {
                let _ = app.emit(
                    status_event,
                    serde_json::json!({ "status": "hop_ok", "hopIndex": hop_index }),
                );
            }
            Err(e) => {
                let _ = app.emit(
                    status_event,
                    serde_json::json!({
                        "status": "hop_failed",
                        "hopIndex": hop_index,
                        "message": e.to_string(),
                    }),
                );
                // Clean up temp sessions
                for s in temp_sessions {
                    let _ = s.disconnect().await;
                }
                return Err(e);
            }
        }
    }

    // Clean up temp sessions
    for s in temp_sessions {
        let _ = s.disconnect().await;
    }

    Ok(())
}

/// Authenticates an SSH hop during post-target testing.
async fn auth_hop(session: &mut SshSession, info: &SshHopInfo) -> Result<(), SshError> {
    match info.auth_type.as_str() {
        "key" => {
            let key_path = info.key_path.as_deref()
                .ok_or_else(|| SshError::AuthFailed("no key path configured".into()))?;
            auth::auth_key(session.handle_mut(), &info.username, key_path, info.passphrase.as_deref()).await
        }
        _ => {
            let password = info.password.as_deref().unwrap_or("");
            auth::auth_password(session.handle_mut(), &info.username, password).await
        }
    }
}

/// Sets up post-target exit routing AFTER the target session is stored in state.sessions.
///
/// For non-TLS proxies: injects the proxy address directly as ALL_PROXY.
/// For TLS proxies: starts a local SOCKS5 relay + SSH remote port forward.
pub async fn setup_post_target_exit(
    app: &AppHandle,
    session_id: &str,
    status_event: &str,
    post_hops: Vec<ResolvedHop>,
) -> Result<ExitProxyInfo, SshError> {
    let state = app.state::<AppState>();

    let _ = app.emit(
        status_event,
        serde_json::json!({
            "status": "connecting",
            "message": "setting up exit routing...",
        }),
    );

    let last_hop = post_hops.last().ok_or_else(|| {
        SshError::ProxyFailed("Empty post-target chain".into())
    })?;

    match last_hop {
        ResolvedHop::Proxy(info) => {
            // Always use relay approach: Termex connects to proxy (handling TLS/auth),
            // then remote port forward exposes a plain SOCKS5 on Target's localhost.
            // This works regardless of whether Target can reach the proxy directly.
            eprintln!(
                ">>> [EXIT_PROXY] Setting up relay: local SOCKS5 → {}:{}",
                info.config.host, info.config.port
            );
            let cancel = CancellationToken::new();

            // Start local SOCKS5 server that routes through the proxy via target's direct-tcpip
            let (local_port, _task) = super::exit_proxy::start_exit_socks5_via_proxy(
                app.clone(),
                session_id.to_string(),
                info.config.clone(),
                cancel.clone(),
            )
            .await?;

            // Request remote port forward on target: target:127.0.0.1:<auto> → local:<local_port>
            let exit_port = {
                let mut sessions = state.sessions.write().await;
                let target = sessions.get_mut(session_id).ok_or_else(|| {
                    SshError::SessionNotFound(session_id.to_string())
                })?;

                let port = target
                    .handle_mut()
                    .tcpip_forward("127.0.0.1", 0)
                    .await
                    .map_err(|e| {
                        SshError::ProxyFailed(format!(
                            "Remote port forward failed (AllowTcpForwarding may be disabled): {}", e
                        ))
                    })? as u16;

                // Register in exit forward registry so ClientHandler bridges forwarded connections
                let reg_key = format!("127.0.0.1:{}", port);
                eprintln!(">>> [EXIT_PROXY] Registering exit forward: {} → local:{}", reg_key, local_port);
                let mut reg = target.exit_forward_registry.write().await;
                reg.insert(reg_key, local_port);

                port
            };

            eprintln!(
                ">>> [EXIT_PROXY] Remote port forward active: target:127.0.0.1:{} → local SOCKS5:{}",
                exit_port, local_port
            );

            Ok(ExitProxyInfo {
                proxy_url: format!("socks5://127.0.0.1:{}", exit_port),
                cancel: Some(cancel),
            })
        }

        ResolvedHop::Ssh(_info) => {
            Err(SshError::ProxyFailed(
                "Post-target chain must end with a Proxy node for exit routing. Add a Proxy after the SSH hop.".into(),
            ))
        }
    }
}

/// Information about the exit proxy to inject into the Target's shell.
pub struct ExitProxyInfo {
    /// The proxy URL to set as ALL_PROXY (e.g., "socks5://host:port")
    pub proxy_url: String,
    /// Optional cancellation token for background tasks
    pub cancel: Option<CancellationToken>,
}

