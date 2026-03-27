use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::ssh::forward;
use crate::state::AppState;
use crate::storage::models::PortForward;

/// Input for creating/updating a port forward rule.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardInput {
    pub server_id: String,
    pub forward_type: String,
    #[serde(default = "default_host")]
    pub local_host: String,
    pub local_port: i32,
    pub remote_host: Option<String>,
    pub remote_port: Option<i32>,
    #[serde(default)]
    pub auto_start: bool,
}

fn default_host() -> String {
    "127.0.0.1".into()
}

/// Lists all port forwards for a server.
#[tauri::command]
pub fn port_forward_list(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<Vec<PortForward>, String> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, server_id, forward_type, local_host, local_port,
                        remote_host, remote_port, auto_start, enabled, created_at
                 FROM port_forwards WHERE server_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![server_id], |row| {
                    let ft: String = row.get(2)?;
                    Ok(PortForward {
                        id: row.get(0)?,
                        server_id: row.get(1)?,
                        forward_type: crate::storage::models::ForwardType::from_str(&ft),
                        local_host: row.get(3)?,
                        local_port: row.get(4)?,
                        remote_host: row.get(5)?,
                        remote_port: row.get(6)?,
                        auto_start: row.get(7)?,
                        enabled: row.get(8)?,
                        created_at: row.get(9)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
        .map_err(|e| e.to_string())
}

/// Saves a new port forward rule.
#[tauri::command]
pub fn port_forward_save(
    state: State<'_, AppState>,
    input: ForwardInput,
) -> Result<PortForward, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO port_forwards (id, server_id, forward_type, local_host,
                    local_port, remote_host, remote_port, auto_start, enabled, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9)",
                rusqlite::params![
                    id,
                    input.server_id,
                    input.forward_type,
                    input.local_host,
                    input.local_port,
                    input.remote_host,
                    input.remote_port,
                    input.auto_start,
                    now,
                ],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(PortForward {
        id,
        server_id: input.server_id,
        forward_type: crate::storage::models::ForwardType::from_str(&input.forward_type),
        local_host: input.local_host,
        local_port: input.local_port,
        remote_host: input.remote_host,
        remote_port: input.remote_port,
        auto_start: input.auto_start,
        enabled: true,
        created_at: now,
    })
}

/// Deletes a port forward rule.
#[tauri::command]
pub fn port_forward_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "DELETE FROM port_forwards WHERE id = ?1",
                rusqlite::params![id],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Starts a local port forward on an active SSH session.
#[tauri::command]
pub async fn port_forward_start(
    state: State<'_, AppState>,
    app: AppHandle,
    session_id: String,
    forward_id: String,
    local_host: String,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
) -> Result<(), String> {
    forward::start_local_forward(
        app,
        session_id,
        forward_id,
        local_host,
        local_port,
        remote_host,
        remote_port,
        &state.forwards,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Stops a port forward.
#[tauri::command]
pub async fn port_forward_stop(
    state: State<'_, AppState>,
    forward_id: String,
) -> Result<(), String> {
    forward::stop_forward(&forward_id, &state.forwards)
        .await
        .map_err(|e| e.to_string())
}
