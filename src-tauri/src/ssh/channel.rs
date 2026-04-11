use russh::ChannelMsg;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

/// Message types for the background channel task.
pub enum ChannelCommand {
    Write(Vec<u8>),
    Resize(u32, u32),
    Close,
}

/// Handle for an active shell channel.
pub struct ChannelHandle {
    /// Send commands (write/resize/close) to the background channel task.
    pub cmd_tx: mpsc::UnboundedSender<ChannelCommand>,
    /// Handle to the background channel task.
    pub task_handle: tokio::task::JoinHandle<()>,
}

/// Spawns a background task that bridges an SSH channel with the Tauri frontend.
///
/// Reads data from the SSH server and emits it as events.
/// Receives user input and resize commands via the channel.
pub fn spawn_channel_task(
    mut channel: russh::Channel<russh::client::Msg>,
    app: AppHandle,
    session_id: String,
) -> ChannelHandle {
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<ChannelCommand>();
    let sid = session_id.clone();

    let task_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                // SSH server → frontend
                msg = channel.wait() => {
                    match msg {
                        Some(ChannelMsg::Data { data }) => {
                            let event = format!("ssh://data/{sid}");
                            let _ = app.emit(&event, data.to_vec());

                            // Hook: record output if session is being recorded
                            if let Some(state) = app.try_state::<crate::state::AppState>() {
                                let text = String::from_utf8_lossy(&data);
                                let within_limit = state.recorder.record_output(&sid, &text).await;
                                if !within_limit {
                                    // Size limit exceeded — auto-stop recording
                                    if let Ok((_, path)) = state.recorder.stop(&sid).await {
                                        let _ = crate::commands::recording::finalize_recording_for_session(
                                            state.inner(), &sid,
                                        ).await;
                                        let _ = app.emit(
                                            &format!("recording://auto-stopped/{sid}"),
                                            serde_json::json!({ "reason": "size_limit" }),
                                        );
                                    }
                                }
                            }
                        }
                        Some(ChannelMsg::ExitStatus { exit_status }) => {
                            let event = format!("ssh://status/{sid}");
                            let _ = app.emit(&event, serde_json::json!({
                                "status": "exited",
                                "message": format!("exit code: {exit_status}"),
                            }));
                            break;
                        }
                        Some(ChannelMsg::Eof) | None => {
                            let event = format!("ssh://status/{sid}");
                            let _ = app.emit(&event, serde_json::json!({
                                "status": "disconnected",
                                "message": "connection closed",
                            }));
                            break;
                        }
                        _ => {}
                    }
                }

                // Frontend → SSH server
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(ChannelCommand::Write(data)) => {
                            if channel.data(&data[..]).await.is_err() {
                                break;
                            }
                        }
                        Some(ChannelCommand::Resize(cols, rows)) => {
                            let _ = channel
                                .window_change(cols, rows, 0, 0)
                                .await;
                        }
                        Some(ChannelCommand::Close) | None => break,
                    }
                }
            }
        }

        let _ = channel.eof().await;
        let _ = channel.close().await;
    });

    ChannelHandle {
        cmd_tx,
        task_handle,
    }
}
