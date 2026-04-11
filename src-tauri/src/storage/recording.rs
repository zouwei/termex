use rusqlite::Connection;

/// Recording metadata stored in the database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingMeta {
    pub id: String,
    pub session_id: String,
    pub server_id: String,
    pub server_name: String,
    pub file_path: String,
    pub file_size: i64,
    pub duration_ms: i64,
    pub cols: u32,
    pub rows: u32,
    pub event_count: i64,
    pub summary: Option<String>,
    pub auto_recorded: bool,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub created_at: String,
}

/// Inserts a recording record (called when recording starts).
pub fn insert(conn: &Connection, meta: &RecordingMeta) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO recordings (id, session_id, server_id, server_name, file_path,
         file_size, duration_ms, cols, rows, event_count, summary, auto_recorded,
         started_at, ended_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        rusqlite::params![
            meta.id,
            meta.session_id,
            meta.server_id,
            meta.server_name,
            meta.file_path,
            meta.file_size,
            meta.duration_ms,
            meta.cols,
            meta.rows,
            meta.event_count,
            meta.summary,
            meta.auto_recorded as i32,
            meta.started_at,
            meta.ended_at,
            meta.created_at,
        ],
    )?;
    Ok(())
}

/// Gets a single recording by ID.
pub fn get(conn: &Connection, id: &str) -> Result<Option<RecordingMeta>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, server_id, server_name, file_path,
                file_size, duration_ms, cols, rows, event_count, summary,
                auto_recorded, started_at, ended_at, created_at
         FROM recordings WHERE id = ?1",
    )?;

    let result = stmt.query_row(rusqlite::params![id], |row| row_to_meta(row));

    match result {
        Ok(meta) => Ok(Some(meta)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Updates recording metadata when recording stops.
pub fn update_on_stop(
    conn: &Connection,
    id: &str,
    file_size: i64,
    duration_ms: i64,
    event_count: i64,
    ended_at: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE recordings SET file_size = ?1, duration_ms = ?2,
         event_count = ?3, ended_at = ?4 WHERE id = ?5",
        rusqlite::params![file_size, duration_ms, event_count, ended_at, id],
    )?;
    Ok(())
}

/// Lists recordings by server (paged, newest first).
pub fn list_by_server(
    conn: &Connection,
    server_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<RecordingMeta>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, server_id, server_name, file_path,
                file_size, duration_ms, cols, rows, event_count, summary,
                auto_recorded, started_at, ended_at, created_at
         FROM recordings WHERE server_id = ?1
         ORDER BY started_at DESC LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt
        .query_map(rusqlite::params![server_id, limit, offset], |row| {
            row_to_meta(row)
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Lists all recordings (paged, newest first).
pub fn list_all(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<RecordingMeta>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, server_id, server_name, file_path,
                file_size, duration_ms, cols, rows, event_count, summary,
                auto_recorded, started_at, ended_at, created_at
         FROM recordings ORDER BY started_at DESC LIMIT ?1 OFFSET ?2",
    )?;

    let rows = stmt
        .query_map(rusqlite::params![limit, offset], |row| row_to_meta(row))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Updates AI summary for a recording.
pub fn update_summary(
    conn: &Connection,
    id: &str,
    summary: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE recordings SET summary = ?1 WHERE id = ?2",
        rusqlite::params![summary, id],
    )?;
    Ok(())
}

/// Deletes a recording record.
pub fn delete(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM recordings WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}

/// Finds recordings to clean up based on retention policy.
/// Returns file paths of expired recordings (caller should delete the files).
pub fn cleanup_expired(
    conn: &Connection,
    retention_days: i64,
) -> Result<Vec<String>, rusqlite::Error> {
    let cutoff = time::OffsetDateTime::now_utc()
        - time::Duration::days(retention_days);
    let cutoff_str = cutoff
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    let mut stmt = conn.prepare(
        "SELECT file_path FROM recordings WHERE started_at < ?1",
    )?;
    let paths: Vec<String> = stmt
        .query_map(rusqlite::params![cutoff_str], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    conn.execute(
        "DELETE FROM recordings WHERE started_at < ?1",
        rusqlite::params![cutoff_str],
    )?;

    Ok(paths)
}

/// Finds the active recording ID for a session (not yet ended).
pub fn find_active_by_session(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<String>, rusqlite::Error> {
    let result = conn.query_row(
        "SELECT id FROM recordings WHERE session_id = ?1 AND ended_at IS NULL
         ORDER BY started_at DESC LIMIT 1",
        rusqlite::params![session_id],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

fn row_to_meta(row: &rusqlite::Row<'_>) -> Result<RecordingMeta, rusqlite::Error> {
    Ok(RecordingMeta {
        id: row.get(0)?,
        session_id: row.get(1)?,
        server_id: row.get(2)?,
        server_name: row.get(3)?,
        file_path: row.get(4)?,
        file_size: row.get(5)?,
        duration_ms: row.get(6)?,
        cols: row.get(7)?,
        rows: row.get(8)?,
        event_count: row.get(9)?,
        summary: row.get(10)?,
        auto_recorded: row.get::<_, i32>(11)? != 0,
        started_at: row.get(12)?,
        ended_at: row.get(13)?,
        created_at: row.get(14)?,
    })
}
