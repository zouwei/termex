use tauri::State;
use crate::state::AppState;

/// Lists audit log entries with optional filtering.
#[tauri::command]
pub fn audit_log_list(
    state: State<'_, AppState>,
    event_type: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<serde_json::Value, String> {
    let limit = limit.unwrap_or(100).min(1000);
    let offset = offset.unwrap_or(0);

    state.db.with_conn(|conn| {
        let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(ref et) = event_type {
            (
                "SELECT id, timestamp, event_type, detail FROM audit_log WHERE event_type = ?1 ORDER BY id DESC LIMIT ?2 OFFSET ?3".to_string(),
                vec![Box::new(et.clone()), Box::new(limit), Box::new(offset)],
            )
        } else {
            (
                "SELECT id, timestamp, event_type, detail FROM audit_log ORDER BY id DESC LIMIT ?1 OFFSET ?2".to_string(),
                vec![Box::new(limit), Box::new(offset)],
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let rows: Vec<serde_json::Value> = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "timestamp": row.get::<_, String>(1)?,
                    "eventType": row.get::<_, String>(2)?,
                    "detail": row.get::<_, Option<String>>(3)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Get total count
        let total: i64 = if let Some(ref et) = event_type {
            conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE event_type = ?1",
                rusqlite::params![et],
                |row| row.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))?
        };

        Ok(serde_json::json!({
            "items": rows,
            "total": total,
        }))
    }).map_err(|e| e.to_string())
}

/// Clears audit log entries older than the configured retention period.
#[tauri::command]
pub fn audit_log_cleanup(
    state: State<'_, AppState>,
    retention_days: Option<i64>,
) -> Result<(), String> {
    crate::audit::cleanup(&state.db, retention_days.unwrap_or(90));
    Ok(())
}
