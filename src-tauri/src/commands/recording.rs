use tauri::State;

use crate::recording::asciicast::AsciicastFile;
use crate::recording::recorder::{self, RecordingInfo};
use crate::state::AppState;
use crate::storage;
use crate::storage::recording::RecordingMeta;

/// Starts recording a terminal session (manual trigger).
#[tauri::command]
pub async fn recording_start(
    state: State<'_, AppState>,
    session_id: String,
    server_id: String,
    server_name: String,
    cols: u32,
    rows: u32,
    title: Option<String>,
) -> Result<String, String> {
    let (rec_id, path) = state
        .recorder
        .start(
            &session_id,
            &server_id,
            &server_name,
            cols,
            rows,
            title,
            false,
            50, // default max MB
        )
        .await
        .map_err(|e| e.to_string())?;

    let now = now_rfc3339();
    let meta = RecordingMeta {
        id: rec_id.clone(),
        session_id,
        server_id,
        server_name,
        file_path: path.to_string_lossy().to_string(),
        file_size: 0,
        duration_ms: 0,
        cols,
        rows,
        event_count: 0,
        summary: None,
        auto_recorded: false,
        started_at: now.clone(),
        ended_at: None,
        created_at: now,
    };
    state
        .db
        .with_conn(|conn| storage::recording::insert(conn, &meta))
        .map_err(|e| e.to_string())?;

    Ok(rec_id)
}

/// Stops recording a terminal session.
#[tauri::command]
pub async fn recording_stop(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<RecordingMeta>, String> {
    finalize_recording_for_session(&state, &session_id).await
}

/// Checks if a session is being recorded.
#[tauri::command]
pub async fn recording_is_active(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    Ok(state.recorder.is_recording(&session_id).await)
}

/// Lists recordings (DB query with optional server filter).
#[tauri::command]
pub async fn recording_list(
    state: State<'_, AppState>,
    server_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<RecordingMeta>, String> {
    let limit = limit.unwrap_or(200);
    let offset = offset.unwrap_or(0);
    state
        .db
        .with_conn(|conn| match &server_id {
            Some(sid) => storage::recording::list_by_server(conn, sid, limit, offset),
            None => storage::recording::list_all(conn, limit, offset),
        })
        .map_err(|e| e.to_string())
}

/// Reads a recording file for playback.
#[tauri::command]
pub fn recording_read(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// Deletes a recording (file + DB record).
#[tauri::command]
pub async fn recording_delete(
    state: State<'_, AppState>,
    recording_id: String,
) -> Result<(), String> {
    let meta = state
        .db
        .with_conn(|conn| storage::recording::get(conn, &recording_id))
        .map_err(|e| e.to_string())?;

    if let Some(ref m) = meta {
        let _ = std::fs::remove_file(&m.file_path);
    }

    state
        .db
        .with_conn(|conn| storage::recording::delete(conn, &recording_id))
        .map_err(|e| e.to_string())
}

/// Cleans up expired recordings.
#[tauri::command]
pub async fn recording_cleanup(
    state: State<'_, AppState>,
    retention_days: Option<i64>,
) -> Result<u32, String> {
    let days = retention_days.unwrap_or(90);
    if days <= 0 {
        return Ok(0);
    }
    let paths = state
        .db
        .with_conn(|conn| storage::recording::cleanup_expired(conn, days))
        .map_err(|e| e.to_string())?;

    let count = paths.len() as u32;
    for path in &paths {
        let _ = std::fs::remove_file(path);
    }
    Ok(count)
}

/// Exports recording as plain text (output events only).
#[tauri::command]
pub fn recording_export_text(path: String) -> Result<String, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let file = AsciicastFile::parse(&content).map_err(|e| e.to_string())?;
    let text: String = file
        .events
        .iter()
        .filter(|e| e.1 == "o")
        .map(|e| e.2.as_str())
        .collect();
    Ok(text)
}

/// Finalizes a recording for a session: stop recorder, update DB.
/// Called by recording_stop command and by ssh_disconnect cleanup.
pub async fn finalize_recording_for_session(
    state: &AppState,
    session_id: &str,
) -> Result<Option<RecordingMeta>, String> {
    if !state.recorder.is_recording(session_id).await {
        return Ok(None);
    }

    let (rec_id, path) = state
        .recorder
        .stop(session_id)
        .await
        .map_err(|e| e.to_string())?;

    let now = now_rfc3339();

    // Read file metadata
    let file_size = std::fs::metadata(&path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let (event_count, duration_ms) = if let Ok(cast) = AsciicastFile::parse(&content) {
        (cast.events.len() as i64, (cast.duration() * 1000.0) as i64)
    } else {
        (0, 0)
    };

    // Try to find recording_id from DB or use the one from recorder
    let db_id = state
        .db
        .with_conn(|conn| storage::recording::find_active_by_session(conn, session_id))
        .unwrap_or(None)
        .unwrap_or(rec_id);

    let _ = state.db.with_conn(|conn| {
        storage::recording::update_on_stop(conn, &db_id, file_size, duration_ms, event_count, &now)
    });

    let meta = state
        .db
        .with_conn(|conn| storage::recording::get(conn, &db_id))
        .map_err(|e| e.to_string())?;

    Ok(meta)
}

/// Generates an AI summary for a recording.
#[tauri::command]
pub async fn recording_summarize(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    recording_id: String,
) -> Result<String, String> {
    use tauri::Emitter;

    let meta = state
        .db
        .with_conn(|conn| storage::recording::get(conn, &recording_id))
        .map_err(|e| e.to_string())?
        .ok_or("recording not found")?;

    // Read and parse the .cast file
    let content = std::fs::read_to_string(&meta.file_path).map_err(|e| e.to_string())?;
    let cast = AsciicastFile::parse(&content).map_err(|e| e.to_string())?;

    // Extract output text and redact sensitive content
    let mut output_text = String::new();
    for event in &cast.events {
        if event.1 == "o" {
            output_text.push_str(&event.2);
        }
    }
    let sanitized = redact_sensitive(&output_text);

    // Truncate to ~50k chars (about 12k tokens)
    let max_chars = 50_000;
    let truncated = if sanitized.len() > max_chars {
        format!(
            "{}\n\n[... truncated {} characters ...]\n\n{}",
            &sanitized[..max_chars / 2],
            sanitized.len() - max_chars,
            &sanitized[sanitized.len() - max_chars / 2..],
        )
    } else {
        sanitized
    };

    // Get default AI provider
    let (provider_type, keychain_id, base_url, model) = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT provider_type, api_key_keychain_id, api_base_url, model
                 FROM ai_providers WHERE is_default = 1 LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
        })
        .map_err(|_| "no default AI provider configured".to_string())?;

    let api_key = keychain_id
        .as_deref()
        .and_then(|k| crate::keychain::get(k).ok())
        .unwrap_or_default();

    let system = "You are a DevOps expert. Analyze the terminal session recording and produce a JSON summary.\n\
        Output ONLY a JSON object with these fields:\n\
        - overview: string (1-3 sentence summary)\n\
        - commands: array of {command, description, timestamp}\n\
        - errors: array of {error, resolution, timestamp}\n\
        - securityActions: array of strings\n\
        - keyFindings: array of strings\n\
        Use the user's language (Chinese if the session is in Chinese).";

    let user_msg = format!(
        "Session on server '{}', duration: {}s\n\n---\n\n{}",
        meta.server_name,
        meta.duration_ms / 1000,
        truncated,
    );

    let response = super::ai::call_ai_provider(
        state.inner(),
        &provider_type,
        &api_key,
        base_url.as_deref(),
        &model,
        system,
        &user_msg,
        Some(4096),
    )
    .await?;

    // Store summary in DB
    state
        .db
        .with_conn(|conn| storage::recording::update_summary(conn, &recording_id, &response))
        .map_err(|e| e.to_string())?;

    // Notify frontend
    let _ = app.emit(
        &format!("recording://summary/{recording_id}"),
        serde_json::json!({ "summary": response, "done": true }),
    );

    Ok(response)
}

/// Redact sensitive content from terminal output.
fn redact_sensitive(text: &str) -> String {
    let patterns = ["password:", "passphrase:", "Password:", "Passphrase:"];
    let mut result = text.to_string();
    for pat in &patterns {
        while let Some(pos) = result.find(pat) {
            let end = result[pos..].find('\n').map(|i| pos + i).unwrap_or(result.len());
            let replacement = format!("{} [REDACTED]", pat);
            result.replace_range(pos..end, &replacement);
        }
    }
    result
}

/// Returns the recordings directory path.
#[tauri::command]
pub fn recording_get_dir() -> Result<String, String> {
    let dir = crate::paths::recordings_dir();
    let _ = std::fs::create_dir_all(&dir);
    Ok(dir.to_string_lossy().to_string())
}

/// Opens the recordings directory in the system file manager.
#[tauri::command]
pub fn recording_open_dir() -> Result<(), String> {
    let dir = crate::paths::recordings_dir();
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
