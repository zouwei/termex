use serde::Serialize;
use tauri::State;

use crate::state::AppState;

/// A key-value setting entry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingEntry {
    pub key: String,
    pub value: String,
}

/// Gets a single setting by key.
#[tauri::command]
pub fn settings_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                rusqlite::params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
        })
        .map_err(|e| e.to_string())
}

/// Gets all settings as key-value pairs.
#[tauri::command]
pub fn settings_get_all(state: State<'_, AppState>) -> Result<Vec<SettingEntry>, String> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(SettingEntry {
                        key: row.get(0)?,
                        value: row.get(1)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
        .map_err(|e| e.to_string())
}

/// Sets a single setting (upsert).
#[tauri::command]
pub fn settings_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                rusqlite::params![key, value, now],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Deletes a setting by key.
#[tauri::command]
pub fn settings_delete(state: State<'_, AppState>, key: String) -> Result<(), String> {
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "DELETE FROM settings WHERE key = ?1",
                rusqlite::params![key],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_entry_serialize() {
        let entry = SettingEntry {
            key: "theme".into(),
            value: "dark".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"key\":\"theme\""));
        assert!(json.contains("\"value\":\"dark\""));
    }
}

/// Helper trait for optional query results.
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
