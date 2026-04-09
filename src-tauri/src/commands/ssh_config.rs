use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ssh::config_parser::{self, ParseError, SshConfigEntry};
use crate::state::AppState;

/// Result of previewing an SSH config file (no side effects).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub entries: Vec<SshConfigEntry>,
    pub errors: Vec<ParseError>,
}

/// Result of importing SSH config entries into the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub total: u32,
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<ImportError>,
}

/// An error for a specific host during import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub host_alias: String,
    pub message: String,
}

/// Previews the contents of an SSH config file without importing.
/// Returns parsed entries and any parse warnings/errors.
#[tauri::command]
pub fn ssh_config_preview(path: Option<String>) -> Result<PreviewResult, String> {
    let config_path = resolve_config_path(path)?;

    let result = config_parser::parse_ssh_config(&config_path)?;

    // Filter out wildcard-only entries for the preview
    let entries: Vec<SshConfigEntry> = result
        .entries
        .into_iter()
        .filter(|e| !e.is_wildcard)
        .collect();

    Ok(PreviewResult {
        entries,
        errors: result.errors,
    })
}

/// Imports selected hosts from an SSH config file into the Termex database.
#[tauri::command]
pub fn ssh_config_import(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
    selected_aliases: Vec<String>,
) -> Result<ImportResult, String> {
    let config_path = resolve_config_path(path)?;
    let parsed = config_parser::parse_ssh_config(&config_path)?;

    state
        .db
        .with_conn(|conn| {
            let now = time::OffsetDateTime::now_utc().to_string();
            let group_id = ensure_import_group(conn, &now)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;

            let mut result = ImportResult {
                total: selected_aliases.len() as u32,
                imported: 0,
                skipped: 0,
                errors: Vec::new(),
            };

            for entry in &parsed.entries {
                if entry.is_wildcard || !selected_aliases.contains(&entry.host_alias) {
                    continue;
                }

                // Dedup: check if server with same host+port+username already exists
                let exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM servers WHERE host = ?1 AND port = ?2 AND username = ?3",
                        rusqlite::params![entry.hostname, entry.port as i32, entry.user],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0)
                    > 0;

                if exists {
                    result.skipped += 1;
                    continue;
                }

                let auth_type = if entry.identity_file.is_some() { "key" } else { "password" };
                let server_id = uuid::Uuid::new_v4().to_string();

                let insert_result = conn.execute(
                    "INSERT INTO servers (id, name, host, port, username, auth_type, key_path,
                                          group_id, encoding, tags, tmux_mode, tmux_close_action,
                                          git_sync_enabled, git_sync_mode, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'UTF-8', '', 'disabled', 'detach', 0, 'notify', ?9, ?9)",
                    rusqlite::params![
                        server_id, entry.host_alias, entry.hostname, entry.port as i32,
                        entry.user, auth_type, entry.identity_file, group_id, now,
                    ],
                );

                match insert_result {
                    Ok(_) => {
                        if let Some(ref jump) = entry.proxy_jump {
                            if let Err(e) = create_proxy_jump_chain(conn, &server_id, jump, &now) {
                                result.errors.push(ImportError {
                                    host_alias: entry.host_alias.clone(),
                                    message: format!("ProxyJump mapping failed: {}", e),
                                });
                            }
                        }
                        if let Some(ref cmd) = entry.proxy_command {
                            if let Err(e) = create_proxy_command_chain(conn, &server_id, cmd, &now) {
                                result.errors.push(ImportError {
                                    host_alias: entry.host_alias.clone(),
                                    message: format!("ProxyCommand mapping failed: {}", e),
                                });
                            }
                        }
                        result.imported += 1;
                    }
                    Err(e) => {
                        result.errors.push(ImportError {
                            host_alias: entry.host_alias.clone(),
                            message: e.to_string(),
                        });
                    }
                }
            }

            Ok(result)
        })
        .map_err(|e| e.to_string())
}

/// Imports servers from a Termius JSON export file.
/// Termius format: { "hosts": [{ "label", "address", "port", "ssh": { "username" } }] }
#[tauri::command]
pub fn ssh_config_import_termius(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<ImportResult, String> {
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?;

    let hosts = json
        .get("hosts")
        .and_then(|h| h.as_array())
        .ok_or("Missing 'hosts' array in Termius JSON")?;

    state
        .db
        .with_conn(|conn| {
            let now = time::OffsetDateTime::now_utc().to_string();
            let group_id = ensure_import_group(conn, &now)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;

            let mut result = ImportResult {
                total: hosts.len() as u32,
                imported: 0,
                skipped: 0,
                errors: Vec::new(),
            };

            for host in hosts {
                let label = host.get("label").and_then(|v| v.as_str()).unwrap_or("unnamed");
                let address = host.get("address").and_then(|v| v.as_str()).unwrap_or("");
                let port = host.get("port").and_then(|v| v.as_i64()).unwrap_or(22) as i32;
                let username = host
                    .get("ssh")
                    .and_then(|s| s.get("username"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("root");

                if address.is_empty() {
                    result.errors.push(ImportError {
                        host_alias: label.to_string(),
                        message: "Missing address".to_string(),
                    });
                    continue;
                }

                // Dedup check
                let exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM servers WHERE host = ?1 AND port = ?2 AND username = ?3",
                        rusqlite::params![address, port, username],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0)
                    > 0;

                if exists {
                    result.skipped += 1;
                    continue;
                }

                let server_id = uuid::Uuid::new_v4().to_string();
                match conn.execute(
                    "INSERT INTO servers (id, name, host, port, username, auth_type, group_id,
                                          encoding, tags, tmux_mode, tmux_close_action,
                                          git_sync_enabled, git_sync_mode, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'password', ?6, 'UTF-8', '', 'disabled', 'detach', 0, 'notify', ?7, ?7)",
                    rusqlite::params![server_id, label, address, port, username, group_id, now],
                ) {
                    Ok(_) => result.imported += 1,
                    Err(e) => result.errors.push(ImportError {
                        host_alias: label.to_string(),
                        message: e.to_string(),
                    }),
                }
            }

            Ok(result)
        })
        .map_err(|e| e.to_string())
}

/// Imports servers from a CSV file.
/// Expected columns: name, host, port, user, key_path (optional header row).
#[tauri::command]
pub fn ssh_config_import_csv(
    state: tauri::State<'_, AppState>,
    file_path: String,
) -> Result<ImportResult, String> {
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let mut lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok(ImportResult { total: 0, imported: 0, skipped: 0, errors: Vec::new() });
    }

    // Skip header if it looks like one
    if let Some(first) = lines.first() {
        let lower = first.to_lowercase();
        if lower.contains("name") && lower.contains("host") {
            lines.remove(0);
        }
    }

    state
        .db
        .with_conn(|conn| {
            let now = time::OffsetDateTime::now_utc().to_string();
            let group_id = ensure_import_group(conn, &now)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;

            let mut result = ImportResult {
                total: lines.len() as u32,
                imported: 0,
                skipped: 0,
                errors: Vec::new(),
            };

            for line in &lines {
                let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if cols.len() < 4 {
                    result.errors.push(ImportError {
                        host_alias: line.to_string(),
                        message: "Expected at least 4 columns: name, host, port, user".to_string(),
                    });
                    continue;
                }

                let name = cols[0];
                let host = cols[1];
                let port: i32 = cols[2].parse().unwrap_or(22);
                let user = cols[3];
                let key_path = cols.get(4).and_then(|s| if s.is_empty() { None } else { Some(*s) });
                let auth_type = if key_path.is_some() { "key" } else { "password" };

                if host.is_empty() {
                    result.errors.push(ImportError {
                        host_alias: name.to_string(),
                        message: "Empty host".to_string(),
                    });
                    continue;
                }

                // Dedup
                let exists: bool = conn
                    .query_row(
                        "SELECT COUNT(*) FROM servers WHERE host = ?1 AND port = ?2 AND username = ?3",
                        rusqlite::params![host, port, user],
                        |row| row.get::<_, i32>(0),
                    )
                    .unwrap_or(0)
                    > 0;

                if exists {
                    result.skipped += 1;
                    continue;
                }

                let server_id = uuid::Uuid::new_v4().to_string();
                match conn.execute(
                    "INSERT INTO servers (id, name, host, port, username, auth_type, key_path, group_id,
                                          encoding, tags, tmux_mode, tmux_close_action,
                                          git_sync_enabled, git_sync_mode, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'UTF-8', '', 'disabled', 'detach', 0, 'notify', ?9, ?9)",
                    rusqlite::params![server_id, name, host, port, user, auth_type, key_path, group_id, now],
                ) {
                    Ok(_) => result.imported += 1,
                    Err(e) => result.errors.push(ImportError {
                        host_alias: name.to_string(),
                        message: e.to_string(),
                    }),
                }
            }

            Ok(result)
        })
        .map_err(|e| e.to_string())
}

// ── Helpers ──

/// Resolves the SSH config path, defaulting to ~/.ssh/config.
fn resolve_config_path(path: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = path {
        Ok(PathBuf::from(p))
    } else {
        dirs::home_dir()
            .map(|h| h.join(".ssh").join("config"))
            .ok_or_else(|| "Could not determine home directory".to_string())
    }
}

/// Ensures the "SSH Config Import" group exists, returns its ID.
fn ensure_import_group(conn: &rusqlite::Connection, now: &str) -> Result<String, String> {
    // Check if group already exists
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM groups WHERE name = 'SSH Config Import'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create the group
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO groups (id, name, color, icon, sort_order, created_at, updated_at)
         VALUES (?1, 'SSH Config Import', '#10b981', 'folder', 0, ?2, ?2)",
        rusqlite::params![id, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

/// Creates connection chain entries for a ProxyJump directive.
/// ProxyJump can be a comma-separated list of hosts: "bastion1,bastion2".
fn create_proxy_jump_chain(
    conn: &rusqlite::Connection,
    server_id: &str,
    proxy_jump: &str,
    now: &str,
) -> Result<(), String> {
    for (position, jump_host) in proxy_jump.split(',').enumerate() {
        let jump_host = jump_host.trim();
        if jump_host.is_empty() || jump_host == "none" {
            continue;
        }

        // Find or create the bastion server
        let bastion_id = find_or_create_bastion(conn, jump_host, now)?;

        let chain_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO connection_chain (id, server_id, position, hop_type, hop_id, phase, created_at)
             VALUES (?1, ?2, ?3, 'ssh', ?4, 'pre', ?5)",
            rusqlite::params![chain_id, server_id, position as i32, bastion_id, now],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Finds an existing server by host or creates a minimal bastion entry.
fn find_or_create_bastion(
    conn: &rusqlite::Connection,
    jump_spec: &str,
    now: &str,
) -> Result<String, String> {
    // Parse jump_spec: can be "host", "host:port", "user@host", "user@host:port"
    let (user, host, port) = parse_jump_spec(jump_spec);

    // Try to find existing server
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM servers WHERE host = ?1 AND port = ?2",
            rusqlite::params![host, port],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create a minimal bastion server entry
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO servers (id, name, host, port, username, auth_type, encoding, tags,
                              tmux_mode, tmux_close_action, git_sync_enabled, git_sync_mode,
                              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'password', 'UTF-8', '', 'disabled', 'detach', 0, 'notify', ?6, ?6)",
        rusqlite::params![
            id,
            format!("bastion-{}", host),
            host,
            port,
            user,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

/// Parses a ProxyJump spec into (user, host, port).
fn parse_jump_spec(spec: &str) -> (String, String, i32) {
    let default_user = whoami::username();
    let (user_part, host_part) = if let Some(at_pos) = spec.find('@') {
        (spec[..at_pos].to_string(), &spec[at_pos + 1..])
    } else {
        (default_user, spec)
    };

    let (host, port) = if let Some(colon_pos) = host_part.rfind(':') {
        let port_str = &host_part[colon_pos + 1..];
        if let Ok(p) = port_str.parse::<i32>() {
            (host_part[..colon_pos].to_string(), p)
        } else {
            (host_part.to_string(), 22)
        }
    } else {
        (host_part.to_string(), 22)
    };

    (user_part, host, port)
}

/// Creates a proxy + chain entry for a ProxyCommand directive.
fn create_proxy_command_chain(
    conn: &rusqlite::Connection,
    server_id: &str,
    command: &str,
    now: &str,
) -> Result<(), String> {
    // Create a proxy entry of type "command"
    let proxy_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO proxies (id, name, proxy_type, host, port, command, created_at, updated_at)
         VALUES (?1, ?2, 'command', '', 0, ?3, ?4, ?4)",
        rusqlite::params![
            proxy_id,
            format!("ProxyCommand for {}", server_id),
            command,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    // Add to connection chain
    let chain_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO connection_chain (id, server_id, position, hop_type, hop_id, phase, created_at)
         VALUES (?1, ?2, 0, 'proxy', ?3, 'pre', ?4)",
        rusqlite::params![chain_id, server_id, proxy_id, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
