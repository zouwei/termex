use std::collections::HashMap;

use crate::state::AppState;
use crate::storage::models::{Snippet, SnippetFolder, SnippetFolderInput, SnippetInput};
use crate::storage::snippet;

// ============================================================
// Snippet Commands
// ============================================================

/// Lists snippets, optionally filtered by folder and/or search query.
#[tauri::command]
pub fn snippet_list(
    state: tauri::State<'_, AppState>,
    folder_id: Option<String>,
    search: Option<String>,
) -> Result<Vec<Snippet>, String> {
    state
        .db
        .with_conn(|conn| {
            snippet::list(conn, folder_id.as_deref(), search.as_deref())
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}

/// Creates a new snippet.
#[tauri::command]
pub fn snippet_create(
    state: tauri::State<'_, AppState>,
    input: SnippetInput,
) -> Result<Snippet, String> {
    state
        .db
        .with_conn(|conn| {
            snippet::create(conn, &input)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}

/// Updates an existing snippet.
#[tauri::command]
pub fn snippet_update(
    state: tauri::State<'_, AppState>,
    id: String,
    input: SnippetInput,
) -> Result<Snippet, String> {
    state
        .db
        .with_conn(|conn| {
            snippet::update(conn, &id, &input)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}

/// Deletes a snippet.
#[tauri::command]
pub fn snippet_delete(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            snippet::delete(conn, &id)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}

/// Executes a snippet: resolves variables, sends the command to the active SSH session.
#[tauri::command]
pub async fn snippet_execute(
    state: tauri::State<'_, AppState>,
    id: String,
    session_id: String,
    variables: HashMap<String, String>,
) -> Result<(), String> {
    // Read snippet from DB
    let snip = state
        .db
        .with_conn(|conn| {
            snippet::get(conn, &id)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Snippet not found".to_string())?;

    let resolved = snippet::resolve_variables(&snip.command, &variables);

    // Send to the SSH session
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or("SSH session not found")?;
    session.write(resolved.as_bytes()).map_err(|e| e.to_string())?;

    // Record usage
    let _ = state.db.with_conn(|conn| {
        snippet::record_usage(conn, &id)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e))
    });

    Ok(())
}

/// Extracts variable names from a snippet's command template.
#[tauri::command]
pub fn snippet_extract_variables(command: String) -> Vec<String> {
    snippet::extract_variables(&command)
}

// ============================================================
// Snippet Folder Commands
// ============================================================

/// Lists all snippet folders.
#[tauri::command]
pub fn snippet_folder_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SnippetFolder>, String> {
    state
        .db
        .with_conn(|conn| {
            snippet::folder_list(conn)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}

/// Creates a new snippet folder.
#[tauri::command]
pub fn snippet_folder_create(
    state: tauri::State<'_, AppState>,
    input: SnippetFolderInput,
) -> Result<SnippetFolder, String> {
    state
        .db
        .with_conn(|conn| {
            snippet::folder_create(conn, &input)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}

/// Updates a snippet folder.
#[tauri::command]
pub fn snippet_folder_update(
    state: tauri::State<'_, AppState>,
    id: String,
    input: SnippetFolderInput,
) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            snippet::folder_update(conn, &id, &input)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}

/// Deletes a snippet folder. Orphaned snippets have folder_id set to NULL.
#[tauri::command]
pub fn snippet_folder_delete(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            snippet::folder_delete(conn, &id)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))
        })
        .map_err(|e| e.to_string())
}
