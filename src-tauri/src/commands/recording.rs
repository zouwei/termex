use tauri::State;

use crate::recording::recorder::{self, RecordingInfo};
use crate::state::AppState;

/// Starts recording a terminal session.
#[tauri::command]
pub async fn recording_start(
    state: State<'_, AppState>,
    session_id: String,
    cols: u32,
    rows: u32,
    title: Option<String>,
) -> Result<String, String> {
    let path = state
        .recorder
        .start(&session_id, cols, rows, title)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Stops recording a terminal session.
#[tauri::command]
pub async fn recording_stop(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<String, String> {
    let path = state
        .recorder
        .stop(&session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Checks if a session is being recorded.
#[tauri::command]
pub async fn recording_is_active(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    Ok(state.recorder.is_recording(&session_id).await)
}

/// Lists all recorded files.
#[tauri::command]
pub fn recording_list() -> Result<Vec<RecordingInfo>, String> {
    recorder::list_recordings().map_err(|e| e.to_string())
}

/// Reads a recording file for playback.
#[tauri::command]
pub fn recording_read(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// Deletes a recording file.
#[tauri::command]
pub fn recording_delete(path: String) -> Result<(), String> {
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}
