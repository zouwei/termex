//! Privacy commands for GDPR compliance.
//! Provides data export and complete erasure functionality.

use tauri::State;
use crate::state::AppState;

/// Erases all user data — GDPR Art.17 (Right to Erasure).
/// Clears database tables, keychain entries, and generated files.
#[tauri::command]
pub fn privacy_erase_all_data(state: State<'_, AppState>) -> Result<(), String> {
    // 1. Clear all database tables (preserve schema)
    state.db.with_conn(|conn| {
        let tables = [
            "servers", "groups", "ssh_keys", "port_forwards",
            "ai_providers", "settings", "known_hosts", "proxies",
            "connection_chain", "audit_log",
        ];
        for table in tables {
            conn.execute(&format!("DELETE FROM {}", table), [])?;
        }
        Ok(())
    }).map_err(|e| format!("Failed to clear database: {}", e))?;

    // 2. Clear OS Keychain entries
    if crate::keychain::is_available() {
        // The keychain stores all credentials in a single JSON entry
        // Storing an empty object effectively erases all credentials
        let _ = crate::keychain::store("__termex_store__", "{}");
    }

    // 3. Delete recording files
    let recordings_dir = crate::paths::recordings_dir();
    if recordings_dir.exists() {
        let _ = std::fs::remove_dir_all(&recordings_dir);
        let _ = std::fs::create_dir_all(&recordings_dir);
    }

    // 4. Delete custom fonts
    let fonts_dir = crate::paths::fonts_dir();
    if fonts_dir.exists() {
        let _ = std::fs::remove_dir_all(&fonts_dir);
        let _ = std::fs::create_dir_all(&fonts_dir);
    }

    // 5. Delete local AI model files
    let models_dir = crate::paths::models_dir();
    if models_dir.exists() {
        let _ = std::fs::remove_dir_all(&models_dir);
        let _ = std::fs::create_dir_all(&models_dir);
    }

    Ok(())
}

/// Counts the total amount of stored user data for the privacy dashboard.
#[tauri::command]
pub fn privacy_data_summary(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state.db.with_conn(|conn| {
        let server_count: i64 = conn.query_row("SELECT COUNT(*) FROM servers", [], |r| r.get(0))?;
        let key_count: i64 = conn.query_row("SELECT COUNT(*) FROM ssh_keys", [], |r| r.get(0))?;
        let provider_count: i64 = conn.query_row("SELECT COUNT(*) FROM ai_providers", [], |r| r.get(0))?;
        let audit_count: i64 = conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0)).unwrap_or(0);
        let known_hosts_count: i64 = conn.query_row("SELECT COUNT(*) FROM known_hosts", [], |r| r.get(0))?;

        Ok(serde_json::json!({
            "servers": server_count,
            "sshKeys": key_count,
            "aiProviders": provider_count,
            "auditEntries": audit_count,
            "knownHosts": known_hosts_count,
        }))
    }).map_err(|e| e.to_string())
}
