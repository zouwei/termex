use std::collections::HashSet;
use std::path::Path;

use rusqlite::Connection;

use super::crypto::{team_decrypt, team_encrypt, TeamCryptoError};
use super::types::{
    SharedGroups, SharedProxies, SharedServerConfig, SharedSnippet, TeamJson,
    TeamSyncResult,
};

/// Sync error types.
#[derive(Debug, thiserror::Error)]
pub enum TeamSyncError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("crypto error: {0}")]
    Crypto(#[from] TeamCryptoError),
    #[error("database error: {0}")]
    Db(String),
    #[error("team.json not found")]
    NoTeamJson,
}

/// Merge decision for LWW conflict resolution.
#[derive(Debug, PartialEq)]
pub enum MergeDecision {
    /// Remote is newer — import into local DB.
    ImportRemote,
    /// Local is newer — export to repo file.
    ExportLocal,
    /// Same timestamp — no action needed.
    NoAction,
}

/// Compares timestamps to decide which version wins (Last Writer Wins).
pub fn merge_decision(local_updated_at: Option<&str>, remote_updated_at: &str) -> MergeDecision {
    match local_updated_at {
        None => MergeDecision::ImportRemote,
        Some(local_ts) => {
            if remote_updated_at > local_ts {
                MergeDecision::ImportRemote
            } else if local_ts > remote_updated_at {
                MergeDecision::ExportLocal
            } else {
                MergeDecision::NoAction
            }
        }
    }
}

/// Reads and parses team.json from the repo.
pub fn read_team_json(repo_path: &Path) -> Result<TeamJson, TeamSyncError> {
    let path = repo_path.join("team.json");
    if !path.exists() {
        return Err(TeamSyncError::NoTeamJson);
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Writes team.json to the repo.
pub fn write_team_json(repo_path: &Path, team: &TeamJson) -> Result<(), TeamSyncError> {
    let path = repo_path.join("team.json");
    let json = serde_json::to_string_pretty(team)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Imports servers from repo into local DB using LWW merge.
///
/// Returns the number of servers imported (inserted or updated).
pub fn import_remote_servers(
    conn: &Connection,
    repo_path: &Path,
    team_key: &[u8; 32],
    team_id: &str,
) -> Result<usize, TeamSyncError> {
    let servers_dir = repo_path.join("servers");
    if !servers_dir.exists() {
        return Ok(0);
    }

    let mut imported = 0;
    for entry in std::fs::read_dir(&servers_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let content = std::fs::read_to_string(&path)?;
            let remote: SharedServerConfig = serde_json::from_str(&content)?;

            // Check if local exists and compare timestamps
            let local_updated = get_server_updated_at(conn, &remote.id);
            let decision = merge_decision(local_updated.as_deref(), &remote.updated_at);

            if decision == MergeDecision::ImportRemote {
                import_single_server(conn, &remote, team_key, team_id)?;
                imported += 1;
            }
        }
    }

    Ok(imported)
}

/// Exports locally shared servers to the repo as JSON files.
///
/// Returns the number of servers exported.
pub fn export_local_shared(
    conn: &Connection,
    repo_path: &Path,
    team_key: &[u8; 32],
    username: &str,
) -> Result<usize, TeamSyncError> {
    let servers_dir = repo_path.join("servers");
    std::fs::create_dir_all(&servers_dir)?;

    let mut exported = 0;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, host, port, username, auth_type,
                    password_keychain_id, passphrase_keychain_id,
                    group_id, startup_cmd, encoding, auto_record,
                    shared_by, shared_at, updated_at
             FROM servers WHERE shared = 1",
        )
        .map_err(|e| TeamSyncError::Db(e.to_string()))?;

    let rows: Vec<SharedServerExportRow> = stmt
        .query_map([], |row| {
            Ok(SharedServerExportRow {
                id: row.get(0)?,
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                auth_type: row.get(5)?,
                password_keychain_id: row.get(6)?,
                passphrase_keychain_id: row.get(7)?,
                group_id: row.get(8)?,
                startup_cmd: row.get(9)?,
                encoding: row.get(10)?,
                auto_record: row.get::<_, i32>(11)? != 0,
                shared_by: row.get(12)?,
                shared_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })
        .map_err(|e| TeamSyncError::Db(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    for row in &rows {
        // Check if repo file exists and compare timestamps
        let file_path = servers_dir.join(format!("{}.json", row.id));
        let remote_ts = if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            let remote: SharedServerConfig = serde_json::from_str(&content)?;
            Some(remote.updated_at)
        } else {
            None
        };

        let decision = merge_decision(remote_ts.as_deref(), &row.updated_at);
        if decision == MergeDecision::ImportRemote || remote_ts.is_none() {
            // Local is newer or file doesn't exist — export
            let password_enc = row
                .password_keychain_id
                .as_deref()
                .and_then(|kid| crate::keychain::get(kid).ok())
                .map(|pwd| team_encrypt(team_key, &pwd))
                .transpose()?;

            let passphrase_enc = row
                .passphrase_keychain_id
                .as_deref()
                .and_then(|kid| crate::keychain::get(kid).ok())
                .map(|pp| team_encrypt(team_key, &pp))
                .transpose()?;

            let config = SharedServerConfig {
                id: row.id.clone(),
                name: row.name.clone(),
                host: row.host.clone(),
                port: row.port,
                username: row.username.clone(),
                auth_type: row.auth_type.clone(),
                password_enc,
                passphrase_enc,
                group_id: row.group_id.clone(),
                tags: String::new(),
                startup_cmd: row.startup_cmd.clone(),
                encoding: row.encoding.clone(),
                auto_record: row.auto_record,
                shared_by: row.shared_by.clone().unwrap_or_else(|| username.to_string()),
                shared_at: row.shared_at.clone().unwrap_or_else(now_rfc3339),
                updated_at: row.updated_at.clone(),
            };

            let json = serde_json::to_string_pretty(&config)?;
            std::fs::write(&file_path, json)?;
            exported += 1;
        }
    }

    Ok(exported)
}

/// Detects servers that were deleted from the remote repo.
///
/// Returns the number of servers deleted locally.
pub fn detect_remote_deletions(
    conn: &Connection,
    repo_path: &Path,
    team_id: &str,
) -> Result<usize, TeamSyncError> {
    let servers_dir = repo_path.join("servers");

    // Get all server IDs in the repo
    let mut remote_ids = HashSet::new();
    if servers_dir.exists() {
        for entry in std::fs::read_dir(&servers_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(stem) = path.file_stem() {
                remote_ids.insert(stem.to_string_lossy().to_string());
            }
        }
    }

    // Find local servers with this team_id that are NOT in remote
    let mut stmt = conn
        .prepare("SELECT id FROM servers WHERE team_id = ?1 AND shared = 1")
        .map_err(|e| TeamSyncError::Db(e.to_string()))?;

    let local_ids: Vec<String> = stmt
        .query_map(rusqlite::params![team_id], |row| row.get(0))
        .map_err(|e| TeamSyncError::Db(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

    let mut deleted = 0;
    for id in &local_ids {
        if !remote_ids.contains(id) {
            // Server was deleted from remote — remove locally
            conn.execute("DELETE FROM servers WHERE id = ?1", rusqlite::params![id])
                .map_err(|e| TeamSyncError::Db(e.to_string()))?;
            deleted += 1;
        }
    }

    Ok(deleted)
}

/// Syncs snippets from repo to local DB (import only, no export yet in v1).
pub fn sync_snippets(
    conn: &Connection,
    repo_path: &Path,
) -> Result<usize, TeamSyncError> {
    let snippets_dir = repo_path.join("snippets");
    if !snippets_dir.exists() {
        return Ok(0);
    }

    let mut imported = 0;
    for entry in std::fs::read_dir(&snippets_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().map(|n| n == "folders.json").unwrap_or(false) {
            continue; // Skip folders file
        }
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let content = std::fs::read_to_string(&path)?;
            let snippet: SharedSnippet = serde_json::from_str(&content)?;

            // Check if exists locally
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM snippets WHERE id = ?1",
                    rusqlite::params![snippet.id],
                    |row| row.get::<_, i32>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);

            if !exists {
                let now = now_rfc3339();
                let tags_json = serde_json::to_string(&snippet.tags).unwrap_or_default();
                conn.execute(
                    "INSERT INTO snippets (id, title, command, description, tags, folder_id,
                     is_favorite, usage_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, ?8)",
                    rusqlite::params![
                        snippet.id, snippet.title, snippet.command, snippet.description,
                        tags_json, snippet.folder_id, now, snippet.updated_at,
                    ],
                )
                .map_err(|e| TeamSyncError::Db(e.to_string()))?;
                imported += 1;
            }
        }
    }

    Ok(imported)
}

/// Syncs groups from repo (groups/groups.json) to local DB.
pub fn sync_groups(
    conn: &Connection,
    repo_path: &Path,
) -> Result<usize, TeamSyncError> {
    let path = repo_path.join("groups").join("groups.json");
    if !path.exists() {
        return Ok(0);
    }

    let content = std::fs::read_to_string(&path)?;
    let groups: SharedGroups = serde_json::from_str(&content)?;

    let mut imported = 0;
    for group in &groups.groups {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM groups WHERE id = ?1",
                rusqlite::params![group.id],
                |row| row.get::<_, i32>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !exists {
            conn.execute(
                "INSERT INTO groups (id, name, color, parent_id, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    group.id, group.name, group.color, group.parent_id, group.sort_order,
                ],
            )
            .map_err(|e| TeamSyncError::Db(e.to_string()))?;
            imported += 1;
        }
    }

    Ok(imported)
}

// ── Internal helpers ──

/// Helper row for export query.
struct SharedServerExportRow {
    id: String,
    name: String,
    host: String,
    port: u16,
    username: String,
    auth_type: String,
    password_keychain_id: Option<String>,
    passphrase_keychain_id: Option<String>,
    group_id: Option<String>,
    startup_cmd: Option<String>,
    encoding: String,
    auto_record: bool,
    shared_by: Option<String>,
    shared_at: Option<String>,
    updated_at: String,
}

/// Gets the `updated_at` timestamp for a local server by ID.
fn get_server_updated_at(conn: &Connection, server_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT updated_at FROM servers WHERE id = ?1",
        rusqlite::params![server_id],
        |row| row.get(0),
    )
    .ok()
}

/// Imports a single server from shared config into local DB.
fn import_single_server(
    conn: &Connection,
    remote: &SharedServerConfig,
    team_key: &[u8; 32],
    team_id: &str,
) -> Result<(), TeamSyncError> {
    // Decrypt credentials
    let password = remote
        .password_enc
        .as_deref()
        .map(|enc| team_decrypt(team_key, enc))
        .transpose()?;

    let passphrase = remote
        .passphrase_enc
        .as_deref()
        .map(|enc| team_decrypt(team_key, enc))
        .transpose()?;

    // Store in keychain
    let password_keychain_id = if let Some(ref pwd) = password {
        let kid = crate::keychain::ssh_password_key(&remote.id);
        let _ = crate::keychain::store(&kid, pwd);
        Some(kid)
    } else {
        None
    };

    let passphrase_keychain_id = if let Some(ref pp) = passphrase {
        let kid = crate::keychain::ssh_passphrase_key(&remote.id);
        let _ = crate::keychain::store(&kid, pp);
        Some(kid)
    } else {
        None
    };

    // Upsert into DB
    conn.execute(
        "INSERT OR REPLACE INTO servers
         (id, name, host, port, username, auth_type,
          password_keychain_id, passphrase_keychain_id,
          group_id, startup_cmd, encoding, auto_record,
          shared, team_id, shared_by, shared_at, updated_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                 1, ?13, ?14, ?15, ?16,
                 COALESCE((SELECT created_at FROM servers WHERE id=?1), ?16))",
        rusqlite::params![
            remote.id, remote.name, remote.host, remote.port,
            remote.username, remote.auth_type,
            password_keychain_id, passphrase_keychain_id,
            remote.group_id, remote.startup_cmd, remote.encoding,
            remote.auto_record as i32,
            team_id, remote.shared_by, remote.shared_at, remote.updated_at,
        ],
    )
    .map_err(|e| TeamSyncError::Db(e.to_string()))?;

    Ok(())
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
