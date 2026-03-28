use serde::Serialize;
use tauri::State;

use crate::keychain;
use crate::state::AppState;

/// A local file system entry.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Returns the user's home directory path.
#[tauri::command]
pub fn local_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "cannot determine home directory".into())
}

/// Lists entries in a local directory.
#[tauri::command]
pub fn local_list_dir(path: String) -> Result<Vec<LocalEntry>, String> {
    let mut entries = Vec::new();

    let read_dir = std::fs::read_dir(&path).map_err(|e| e.to_string())?;

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        entries.push(LocalEntry {
            name,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
        });
    }

    // Sort: directories first, then by name
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });

    Ok(entries)
}

/// Security status information.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityStatus {
    pub keychain_available: bool,
    pub keychain_credential_count: i32,
    pub protection_mode: String,
}

/// Opens a URL in the system default browser.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| e.to_string())
}

/// Returns the current security/keychain status.
#[tauri::command]
pub fn security_status(state: State<'_, AppState>) -> Result<SecurityStatus, String> {
    let available = keychain::is_available();

    let count = if available {
        // Count credentials in keychain by checking DB for keychain_id references
        let server_count: i32 = state.db.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM servers WHERE password_keychain_id IS NOT NULL",
                [], |row| row.get(0),
            )
        }).unwrap_or(0);
        let ai_count: i32 = state.db.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM ai_providers WHERE api_key_keychain_id IS NOT NULL",
                [], |row| row.get(0),
            )
        }).unwrap_or(0);
        server_count + ai_count
    } else {
        0
    };

    Ok(SecurityStatus {
        keychain_available: available,
        keychain_credential_count: count,
        protection_mode: if available { "keychain".into() } else { "local".into() },
    })
}
