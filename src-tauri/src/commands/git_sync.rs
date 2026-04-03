//! Git Auto Sync commands — reverse port forwarding, script deployment, local pull.

use tauri::State;

use crate::ssh::reverse_forward::calculate_sync_port;
use crate::state::AppState;

/// Deploys the git-sync.sh script to the remote server via exec_command.
/// Only called when git_sync_enabled is true and SSH is connected.
#[tauri::command]
pub async fn git_sync_deploy(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;

    // Get server_id from session metadata to calculate sync port
    let server_id = session_id.clone(); // We use session_id as key; server_id comes from frontend
    let sync_port = calculate_sync_port(&server_id);

    let script = include_str!("../../assets/git-sync.sh");
    let deploy_cmd = format!(
        "mkdir -p ~/.termex && cat > ~/.termex/git-sync.sh << 'TERMEX_SYNC_EOF'\n{}\nTERMEX_SYNC_EOF\nchmod +x ~/.termex/git-sync.sh && echo {} > ~/.termex/sync-port",
        script, sync_port,
    );

    let (stdout, exit_code) = session
        .exec_command(&deploy_cmd)
        .await
        .map_err(|e| e.to_string())?;

    if exit_code != 0 {
        return Err(format!(
            "Failed to deploy git-sync.sh (exit {}): {}",
            exit_code, stdout
        ));
    }

    Ok(())
}

/// Sets up reverse port forwarding for Git Sync notifications.
/// Remote 127.0.0.1:{port} → forwarded through SSH → handled by ClientHandler.
#[tauri::command]
pub async fn git_sync_setup_tunnel(
    state: State<'_, AppState>,
    session_id: String,
    server_id: String,
) -> Result<u32, String> {
    let port = calculate_sync_port(&server_id);

    // Register in reverse forward registry
    {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
        let mut reg = state.reverse_forward_registry.write().await;
        reg.register("127.0.0.1", port, server_id.clone(), tx);

        // Spawn a background task to drain the channel (actual handling is in ClientHandler)
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                // Data is handled in ClientHandler::server_channel_open_forwarded_tcpip
            }
        });
    }

    // Request remote port forwarding
    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;

    let returned_port = session
        .handle_mut()
        .tcpip_forward("127.0.0.1", port)
        .await
        .map_err(|e| format!("Failed to setup reverse forward: {}", e))?;

    Ok(returned_port)
}

/// Executes a local git pull in the specified directory.
#[tauri::command]
pub async fn git_sync_pull(local_path: String) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(["pull"])
        .current_dir(&local_path)
        .output()
        .await
        .map_err(|e| format!("Failed to run git pull: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("git pull failed: {}{}", stdout, stderr));
    }

    Ok(stdout)
}
