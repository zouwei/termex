use std::collections::HashMap;
use std::sync::Arc;

use russh::ChannelMsg;
use tauri::Manager;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Tracks the state of an active port forward.
pub struct ActiveForward {
    /// Unique ID matching the DB record.
    pub id: String,
    /// Cancellation token for stopping the forward.
    pub cancel: CancellationToken,
    /// Background task handle.
    pub task: tokio::task::JoinHandle<()>,
}

impl ActiveForward {
    /// Stops this forwarding.
    pub fn stop(self) {
        self.cancel.cancel();
        self.task.abort();
    }
}

/// Registry of all active port forwards keyed by forward ID.
pub type ForwardRegistry = Arc<RwLock<HashMap<String, ActiveForward>>>;

/// Creates a new empty forward registry.
pub fn new_registry() -> ForwardRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Starts a local port forward (`-L`): listens on `local_host:local_port` and
/// tunnels each incoming TCP connection to `remote_host:remote_port` via SSH.
pub async fn start_local_forward(
    app: tauri::AppHandle,
    session_id: String,
    forward_id: String,
    local_host: String,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
    registry: &ForwardRegistry,
) -> Result<(), super::SshError> {
    let listener = TcpListener::bind(format!("{local_host}:{local_port}"))
        .await
        .map_err(super::SshError::Io)?;

    let cancel = CancellationToken::new();
    let cancel_child = cancel.clone();
    let fid = forward_id.clone();

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((local_stream, _)) => {
                            let rh = remote_host.clone();
                            let sid = session_id.clone();
                            let app2 = app.clone();

                            tokio::spawn(async move {
                                handle_forward_connection(
                                    app2, sid, local_stream, rh, remote_port,
                                ).await;
                            });
                        }
                        Err(_) => break,
                    }
                }
                _ = cancel_child.cancelled() => break,
            }
        }
    });

    let active = ActiveForward {
        id: fid.clone(),
        cancel,
        task,
    };
    registry.write().await.insert(fid, active);

    Ok(())
}

/// Handles a single forwarded TCP connection.
async fn handle_forward_connection(
    app: tauri::AppHandle,
    session_id: String,
    local_stream: tokio::net::TcpStream,
    remote_host: String,
    remote_port: u16,
) {
    let state = app.state::<crate::state::AppState>();
    let sessions = state.sessions.read().await;
    let Some(ssh) = sessions.get(&session_id) else {
        return;
    };
    let channel = match ssh
        .handle()
        .channel_open_direct_tcpip(&remote_host, remote_port as u32, "127.0.0.1", 0)
        .await
    {
        Ok(ch) => ch,
        Err(_) => return,
    };
    drop(sessions);

    bridge_streams(local_stream, channel).await;
}

/// Bridges a local TCP stream with an SSH channel bidirectionally using `select!`.
async fn bridge_streams(
    local_stream: tokio::net::TcpStream,
    mut channel: russh::Channel<russh::client::Msg>,
) {
    let (mut local_rd, mut local_wr) = local_stream.into_split();
    let mut buf = vec![0u8; 32768];

    loop {
        tokio::select! {
            // local → remote (SSH channel)
            result = local_rd.read(&mut buf) => {
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if channel.data(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            }
            // remote (SSH channel) → local
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        if local_wr.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::Eof) | None => break,
                    _ => {}
                }
            }
        }
    }

    let _ = channel.close().await;
}

/// Starts a dynamic port forward (`-D`): listens on `local_host:local_port`,
/// runs a SOCKS5 server, and tunnels each connection via SSH direct-tcpip.
pub async fn start_dynamic_forward(
    app: tauri::AppHandle,
    session_id: String,
    forward_id: String,
    local_host: String,
    local_port: u16,
    registry: &ForwardRegistry,
) -> Result<(), super::SshError> {
    let listener = TcpListener::bind(format!("{local_host}:{local_port}"))
        .await
        .map_err(super::SshError::Io)?;

    let cancel = CancellationToken::new();
    let cancel_child = cancel.clone();
    let fid = forward_id.clone();

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((local_stream, _)) => {
                            let sid = session_id.clone();
                            let app2 = app.clone();
                            tokio::spawn(async move {
                                handle_dynamic_connection(app2, sid, local_stream).await;
                            });
                        }
                        Err(_) => break,
                    }
                }
                _ = cancel_child.cancelled() => break,
            }
        }
    });

    let active = ActiveForward {
        id: fid.clone(),
        cancel,
        task,
    };
    registry.write().await.insert(fid, active);

    Ok(())
}

/// Handles a single dynamic-forwarded TCP connection:
/// SOCKS5 handshake → SSH direct-tcpip → bridge.
async fn handle_dynamic_connection(
    app: tauri::AppHandle,
    session_id: String,
    mut local_stream: tokio::net::TcpStream,
) {
    use super::socks5;

    // SOCKS5 handshake to get target address
    let (target_host, target_port) = match socks5::socks5_handshake(&mut local_stream).await {
        Ok(addr) => addr,
        Err(_) => return,
    };

    // Open SSH direct-tcpip channel to the target
    let state = app.state::<crate::state::AppState>();
    let sessions = state.sessions.read().await;
    let Some(ssh) = sessions.get(&session_id) else {
        let _ = socks5::socks5_reply_failure(&mut local_stream, socks5::REPLY_GENERAL_FAILURE).await;
        return;
    };

    let channel = match ssh
        .handle()
        .channel_open_direct_tcpip(&target_host, target_port as u32, "127.0.0.1", 0)
        .await
    {
        Ok(ch) => ch,
        Err(_) => {
            let _ = socks5::socks5_reply_failure(&mut local_stream, socks5::REPLY_HOST_UNREACHABLE).await;
            return;
        }
    };
    drop(sessions);

    // Send SOCKS5 success reply
    if socks5::socks5_reply_success(&mut local_stream).await.is_err() {
        return;
    }

    // Bridge the streams
    bridge_streams(local_stream, channel).await;
}

/// Stops a forwarding by its ID.
pub async fn stop_forward(
    forward_id: &str,
    registry: &ForwardRegistry,
) -> Result<(), super::SshError> {
    let active = registry
        .write()
        .await
        .remove(forward_id)
        .ok_or_else(|| super::SshError::SessionNotFound(forward_id.to_string()))?;
    active.stop();
    Ok(())
}