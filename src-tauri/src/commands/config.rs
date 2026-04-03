use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::crypto::{aes, kdf};
use crate::state::AppState;

/// Magic bytes identifying a `.termex` export file.
const MAGIC: &[u8; 4] = b"TMEX";
/// Current export format version.
const FORMAT_VERSION: u16 = 1;

/// Export payload containing all user data.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportPayload {
    servers: Vec<serde_json::Value>,
    groups: Vec<serde_json::Value>,
    port_forwards: Vec<serde_json::Value>,
    settings: Vec<serde_json::Value>,
}

/// Exports user data to an encrypted `.termex` file.
///
/// File format: `TMEX(4B) | version(2B LE) | salt(16B) | ciphertext | tag(16B)`
/// The payload is JSON → gzip → AES-256-GCM, keyed via Argon2id from the export password.
#[tauri::command]
pub fn config_export(
    state: State<'_, AppState>,
    file_path: String,
    password: String,
    server_ids: Option<Vec<String>>,
) -> Result<(), String> {
    // Collect data — optionally filtered by server_ids
    let payload = state
        .db
        .with_conn(|conn| {
            let (servers, group_ids) = if let Some(ref ids) = server_ids {
                let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
                let sql = format!("SELECT * FROM servers WHERE id IN ({})", placeholders.join(","));
                let mut stmt = conn.prepare(&sql)?;
                let params: Vec<&dyn rusqlite::types::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
                let servers = query_stmt_json(&mut stmt, &params)?;
                // Collect group_ids referenced by exported servers
                let gids: Vec<String> = servers.iter()
                    .filter_map(|s| s["group_id"].as_str().map(|s| s.to_string()))
                    .collect();
                (servers, Some(gids))
            } else {
                (query_all_json(conn, "SELECT * FROM servers")?, None)
            };

            let groups = if let Some(ref gids) = group_ids {
                if gids.is_empty() {
                    Vec::new()
                } else {
                    let placeholders: Vec<String> = gids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
                    let sql = format!("SELECT * FROM groups WHERE id IN ({})", placeholders.join(","));
                    let mut stmt = conn.prepare(&sql)?;
                    let params: Vec<&dyn rusqlite::types::ToSql> = gids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
                    query_stmt_json(&mut stmt, &params)?
                }
            } else {
                query_all_json(conn, "SELECT * FROM groups")?
            };

            let port_forwards = if let Some(ref ids) = server_ids {
                if ids.is_empty() {
                    Vec::new()
                } else {
                    let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
                    let sql = format!("SELECT * FROM port_forwards WHERE server_id IN ({})", placeholders.join(","));
                    let mut stmt = conn.prepare(&sql)?;
                    let params: Vec<&dyn rusqlite::types::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
                    query_stmt_json(&mut stmt, &params)?
                }
            } else {
                query_all_json(conn, "SELECT * FROM port_forwards")?
            };

            // Settings only exported for full backup
            let settings = if server_ids.is_none() {
                query_all_json(conn, "SELECT * FROM settings")?
            } else {
                Vec::new()
            };

            Ok(ExportPayload {
                servers,
                groups,
                port_forwards,
                settings,
            })
        })
        .map_err(|e| e.to_string())?;

    let json = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;

    // Compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;

    // Derive key from export password
    let (key, salt) = kdf::derive_key_new(&password).map_err(|e| e.to_string())?;

    // Encrypt
    let encrypted = aes::encrypt(&key, &compressed).map_err(|e| e.to_string())?;

    // Write file: magic + version + salt + ciphertext
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    file.write_all(MAGIC).map_err(|e| e.to_string())?;
    file.write_all(&FORMAT_VERSION.to_le_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(&salt).map_err(|e| e.to_string())?;
    file.write_all(&encrypted).map_err(|e| e.to_string())?;

    Ok(())
}

/// Imports user data from an encrypted `.termex` file.
#[tauri::command]
pub fn config_import(
    state: State<'_, AppState>,
    file_path: String,
    password: String,
    on_conflict: String,
) -> Result<ImportResult, String> {
    let data = std::fs::read(&file_path).map_err(|e| e.to_string())?;

    // Validate magic + version
    if data.len() < 22 || &data[0..4] != MAGIC {
        return Err("invalid .termex file format".into());
    }
    let _version = u16::from_le_bytes([data[4], data[5]]);
    let salt: [u8; 16] = data[6..22].try_into().map_err(|_| "invalid salt")?;
    let encrypted = &data[22..];

    // Derive key and decrypt
    let key = kdf::derive_key(&password, &salt).map_err(|e| e.to_string())?;
    let compressed = aes::decrypt(&key, encrypted).map_err(|_| "wrong password or corrupted file")?;

    // Decompress
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut json_bytes = Vec::new();
    decoder
        .read_to_end(&mut json_bytes)
        .map_err(|e| e.to_string())?;

    let payload: ExportPayload =
        serde_json::from_slice(&json_bytes).map_err(|e| e.to_string())?;

    // Import data
    let mut imported = 0u32;
    let mut skipped = 0u32;

    state
        .db
        .with_conn(|conn| {
            // Import groups
            for group in &payload.groups {
                let id = group["id"].as_str().unwrap_or_default();
                let exists = conn
                    .query_row(
                        "SELECT COUNT(*) FROM groups WHERE id = ?1",
                        rusqlite::params![id],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0)
                    > 0;

                if exists && on_conflict == "skip" {
                    skipped += 1;
                    continue;
                }

                let sql = if exists {
                    "UPDATE groups SET name=?2, color=?3, icon=?4, parent_id=?5, sort_order=?6, updated_at=?7 WHERE id=?1"
                } else {
                    "INSERT INTO groups (id, name, color, icon, parent_id, sort_order, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7)"
                };

                conn.execute(
                    sql,
                    rusqlite::params![
                        id,
                        group["name"].as_str().unwrap_or(""),
                        group["color"].as_str().unwrap_or("#6366f1"),
                        group["icon"].as_str().unwrap_or("folder"),
                        group["parent_id"].as_str(),
                        group["sort_order"].as_i64().unwrap_or(0),
                        time::OffsetDateTime::now_utc().to_string(),
                    ],
                )?;
                imported += 1;
            }

            // Import servers
            for server in &payload.servers {
                let id = server["id"].as_str().unwrap_or_default();
                let exists = conn
                    .query_row(
                        "SELECT COUNT(*) FROM servers WHERE id = ?1",
                        rusqlite::params![id],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0)
                    > 0;

                if exists && on_conflict == "skip" {
                    skipped += 1;
                    continue;
                }

                if exists {
                    conn.execute(
                        "DELETE FROM servers WHERE id = ?1",
                        rusqlite::params![id],
                    )?;
                }

                let now = time::OffsetDateTime::now_utc().to_string();
                conn.execute(
                    "INSERT INTO servers (id, name, host, port, username, auth_type,
                        password_enc, key_path, passphrase_enc, group_id, sort_order,
                        proxy_id, startup_cmd, encoding, tags, created_at, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
                    rusqlite::params![
                        id,
                        server["name"].as_str().unwrap_or(""),
                        server["host"].as_str().unwrap_or(""),
                        server["port"].as_i64().unwrap_or(22),
                        server["username"].as_str().unwrap_or("root"),
                        server["auth_type"].as_str().unwrap_or("password"),
                        server["password_enc"].as_str().map(|_| Vec::<u8>::new()),
                        server["key_path"].as_str(),
                        Option::<Vec<u8>>::None,
                        server["group_id"].as_str(),
                        server["sort_order"].as_i64().unwrap_or(0),
                        server["proxy_id"].as_str(),
                        server["startup_cmd"].as_str(),
                        server["encoding"].as_str().unwrap_or("UTF-8"),
                        server["tags"].as_str().unwrap_or("[]"),
                        now,
                        now,
                    ],
                )?;
                imported += 1;
            }

            Ok(())
        })
        .map_err(|e| e.to_string())?;

    Ok(ImportResult { imported, skipped })
}

/// Result of a config import operation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: u32,
    pub skipped: u32,
}

/// Helper: executes a query and returns all rows as JSON values.
fn query_all_json(
    conn: &rusqlite::Connection,
    sql: &str,
) -> Result<Vec<serde_json::Value>, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("").to_string())
        .collect();

    let rows = stmt
        .query_map([], |row| {
            let mut map = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => serde_json::Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(n)) => {
                        serde_json::Value::Number(n.into())
                    }
                    Ok(rusqlite::types::ValueRef::Real(f)) => serde_json::json!(f),
                    Ok(rusqlite::types::ValueRef::Text(s)) => {
                        serde_json::Value::String(
                            String::from_utf8_lossy(s).to_string(),
                        )
                    }
                    Ok(rusqlite::types::ValueRef::Blob(_)) => {
                        // Skip binary blobs (encrypted data) in export
                        serde_json::Value::Null
                    }
                    Err(_) => serde_json::Value::Null,
                };
                map.insert(name.clone(), val);
            }
            Ok(serde_json::Value::Object(map))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

/// Helper: executes a prepared statement with params and returns all rows as JSON values.
fn query_stmt_json(
    stmt: &mut rusqlite::Statement,
    params: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<serde_json::Value>, rusqlite::Error> {
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("").to_string())
        .collect();

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            let mut map = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => serde_json::Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(n)) => {
                        serde_json::Value::Number(n.into())
                    }
                    Ok(rusqlite::types::ValueRef::Real(f)) => serde_json::json!(f),
                    Ok(rusqlite::types::ValueRef::Text(s)) => {
                        serde_json::Value::String(
                            String::from_utf8_lossy(s).to_string(),
                        )
                    }
                    Ok(rusqlite::types::ValueRef::Blob(_)) => serde_json::Value::Null,
                    Err(_) => serde_json::Value::Null,
                };
                map.insert(name.clone(), val);
            }
            Ok(serde_json::Value::Object(map))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}