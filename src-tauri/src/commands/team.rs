use std::path::PathBuf;

use tauri::{Emitter, State};

use crate::state::AppState;
use crate::storage;
use crate::team::crypto::{
    create_verify_token, derive_team_key, generate_team_salt, re_encrypt,
    team_encrypt, verify_passphrase,
};
use crate::team::git::{PullResult, TeamRepo};
use crate::team::sync;
use crate::team::types::*;

/// Helper: get current team username from settings.
fn get_team_username(state: &AppState) -> Result<String, String> {
    state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT value FROM settings WHERE key = 'team_username'",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .map_err(|_| "not in a team".to_string())
}

/// Helper: get team role from settings.
fn get_team_role(state: &AppState) -> Result<String, String> {
    state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT value FROM settings WHERE key = 'team_role'",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .map_err(|_| "not in a team".to_string())
}

/// Helper: check if current role meets the required level.
fn check_role(state: &AppState, required: &str) -> Result<(), String> {
    let role = get_team_role(state)?;
    let role_level = match role.as_str() {
        "admin" => 3,
        "member" => 2,
        "readonly" => 1,
        _ => 0,
    };
    let required_level = match required {
        "admin" => 3,
        "member" => 2,
        "readonly" => 1,
        _ => 0,
    };
    if role_level >= required_level {
        Ok(())
    } else {
        Err(format!(
            "insufficient permissions: requires {required}, you are {role}"
        ))
    }
}

/// Helper: load git auth config from settings.
fn load_git_auth(state: &AppState) -> Result<GitAuthConfig, String> {
    let get = |key: &str| -> Option<String> {
        state
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    rusqlite::params![key],
                    |row| row.get(0),
                )
            })
            .ok()
    };

    Ok(GitAuthConfig {
        auth_type: get("team_git_auth_type").unwrap_or_else(|| "ssh_key".to_string()),
        ssh_key_path: get("team_git_ssh_key"),
        ssh_passphrase: get("team_git_ssh_passphrase")
            .and_then(|kid| crate::keychain::get(&kid).ok()),
        token: get("team_git_token_keychain_id")
            .and_then(|kid| crate::keychain::get(&kid).ok()),
        username: get("team_git_username"),
        password: get("team_git_password_keychain_id")
            .and_then(|kid| crate::keychain::get(&kid).ok()),
    })
}

/// Helper: save a setting.
fn set_setting(state: &AppState, key: &str, value: &str) {
    let _ = state.db.with_conn(|conn| {
        let now = now_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, value, now],
        )
    });
}

/// Helper: save git auth config to settings.
fn save_git_auth(state: &AppState, auth: &GitAuthConfig) {
    set_setting(state, "team_git_auth_type", &auth.auth_type);
    if let Some(ref p) = auth.ssh_key_path {
        set_setting(state, "team_git_ssh_key", p);
    }
    if let Some(ref p) = auth.ssh_passphrase {
        let kid = "termex:team:git:ssh_passphrase";
        let _ = crate::keychain::store(kid, p);
        set_setting(state, "team_git_ssh_passphrase", kid);
    }
    if let Some(ref t) = auth.token {
        let kid = "termex:team:git:token";
        let _ = crate::keychain::store(kid, t);
        set_setting(state, "team_git_token_keychain_id", kid);
    }
    if let Some(ref u) = auth.username {
        set_setting(state, "team_git_username", u);
    }
    if let Some(ref p) = auth.password {
        let kid = "termex:team:git:password";
        let _ = crate::keychain::store(kid, p);
        set_setting(state, "team_git_password_keychain_id", kid);
    }
}

/// Team repo default path.
fn default_repo_path() -> PathBuf {
    crate::paths::data_dir().join("team-repo")
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

// ── Tauri Commands ──

/// Creates a new team: init Git repo, generate team.json, push.
#[tauri::command]
pub async fn team_create(
    state: State<'_, AppState>,
    name: String,
    passphrase: String,
    repo_url: String,
    username: String,
    git_auth: GitAuthConfig,
) -> Result<TeamInfo, String> {
    if passphrase.len() < 8 {
        return Err("passphrase must be at least 8 characters".to_string());
    }

    let repo_path = default_repo_path();
    if repo_path.exists() {
        return Err("team repo already exists — leave current team first".to_string());
    }
    std::fs::create_dir_all(&repo_path).map_err(|e| e.to_string())?;

    // Generate salt + derive key
    let salt = generate_team_salt().map_err(|e| e.to_string())?;
    let key = derive_team_key(&passphrase, &salt).map_err(|e| e.to_string())?;
    let verify = create_verify_token(&key).map_err(|e| e.to_string())?;

    let now = now_rfc3339();
    let device_id = uuid::Uuid::new_v4().to_string();

    // Create team.json
    let team = TeamJson {
        version: 1,
        name: name.clone(),
        salt: hex::encode(salt),
        verify,
        members: vec![TeamMemberEntry {
            username: username.clone(),
            role: "admin".to_string(),
            joined_at: now.clone(),
            device_id,
        }],
        settings: TeamSettings::default(),
    };

    sync::write_team_json(&repo_path, &team).map_err(|e| e.to_string())?;

    // Create directory structure
    for dir in &["servers", "groups", "snippets", "proxies"] {
        let d = repo_path.join(dir);
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        std::fs::write(d.join(".gitkeep"), "").map_err(|e| e.to_string())?;
    }
    std::fs::write(repo_path.join(".gitignore"), "*.tmp\n*.lock\n.sync_state\n")
        .map_err(|e| e.to_string())?;

    // Git init + push
    TeamRepo::init_and_push(&repo_path, &repo_url, &username, &git_auth)
        .map_err(|e| e.to_string())?;

    // Store team key in memory
    *state.team_key.write().await = Some(key);
    *state.team_repo_path.write().await = Some(repo_path);

    // Save settings
    set_setting(&state, "team_name", &name);
    set_setting(&state, "team_repo_url", &repo_url);
    set_setting(&state, "team_repo_path", &default_repo_path().to_string_lossy());
    set_setting(&state, "team_username", &username);
    set_setting(&state, "team_role", "admin");
    save_git_auth(&state, &git_auth);

    crate::audit::log(&state.db, crate::audit::AuditEvent::TeamCreate { name: name.clone() });

    Ok(TeamInfo {
        name,
        repo_url,
        role: "admin".to_string(),
        member_count: 1,
        created_at: now,
    })
}

/// Joins an existing team: clone repo, verify passphrase, import configs.
#[tauri::command]
pub async fn team_join(
    state: State<'_, AppState>,
    repo_url: String,
    passphrase: String,
    username: String,
    git_auth: GitAuthConfig,
) -> Result<TeamInfo, String> {
    let repo_path = default_repo_path();
    if repo_path.exists() {
        return Err("team repo already exists — leave current team first".to_string());
    }

    // Clone
    TeamRepo::clone_repo(&repo_url, &repo_path, &git_auth).map_err(|e| e.to_string())?;

    // Read team.json
    let mut team = sync::read_team_json(&repo_path).map_err(|e| {
        let _ = std::fs::remove_dir_all(&repo_path);
        e.to_string()
    })?;

    // Derive key + verify
    let salt_bytes = hex::decode(&team.salt).map_err(|_| {
        let _ = std::fs::remove_dir_all(&repo_path);
        "invalid salt in team.json".to_string()
    })?;
    let salt: [u8; 16] = salt_bytes.try_into().map_err(|_| {
        let _ = std::fs::remove_dir_all(&repo_path);
        "invalid salt length".to_string()
    })?;
    let key = derive_team_key(&passphrase, &salt).map_err(|e| {
        let _ = std::fs::remove_dir_all(&repo_path);
        e.to_string()
    })?;

    if !verify_passphrase(&key, &team.verify) {
        let _ = std::fs::remove_dir_all(&repo_path);
        return Err("incorrect team passphrase".to_string());
    }

    // Determine role
    let role = if let Some(existing) = team.members.iter().find(|m| m.username == username) {
        existing.role.clone()
    } else {
        // Add self as member
        let now = now_rfc3339();
        team.members.push(TeamMemberEntry {
            username: username.clone(),
            role: "member".to_string(),
            joined_at: now,
            device_id: uuid::Uuid::new_v4().to_string(),
        });
        sync::write_team_json(&repo_path, &team).map_err(|e| e.to_string())?;

        // Commit + push join
        let repo = TeamRepo::open(&repo_path).map_err(|e| e.to_string())?;
        let _ = repo.commit_and_push(
            &format!("join: {username}"),
            &username,
            &git_auth,
        );
        "member".to_string()
    };

    // Import servers into local DB
    let team_id = format!("team:{}", team.name);
    state
        .db
        .with_conn(|conn| {
            sync::import_remote_servers(conn, &repo_path, &key, &team_id)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })
        .map_err(|e| e.to_string())?;

    // Store state
    *state.team_key.write().await = Some(key);
    *state.team_repo_path.write().await = Some(repo_path);

    set_setting(&state, "team_name", &team.name);
    set_setting(&state, "team_repo_url", &repo_url);
    set_setting(&state, "team_repo_path", &default_repo_path().to_string_lossy());
    set_setting(&state, "team_username", &username);
    set_setting(&state, "team_role", &role);
    save_git_auth(&state, &git_auth);

    crate::audit::log(&state.db, crate::audit::AuditEvent::TeamJoin {
        name: team.name.clone(),
        username: username.clone(),
    });

    Ok(TeamInfo {
        name: team.name.clone(),
        repo_url,
        role,
        member_count: team.members.len(),
        created_at: team
            .members
            .first()
            .map(|m| m.joined_at.clone())
            .unwrap_or_default(),
    })
}

/// Syncs team: pull + merge + export + push.
#[tauri::command]
pub async fn team_sync(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<TeamSyncResult, String> {
    let repo_path = state.team_repo_path.read().await.clone()
        .ok_or("not in a team")?;
    let key = state.team_key.read().await.clone()
        .ok_or("team key not loaded — enter passphrase first")?;

    let auth = load_git_auth(&state)?;
    let username = get_team_username(&state)?;
    let team_name = state
        .db
        .with_conn(|conn| {
            conn.query_row("SELECT value FROM settings WHERE key='team_name'", [], |r| r.get::<_, String>(0))
        })
        .unwrap_or_default();
    let team_id = format!("team:{team_name}");

    let repo = TeamRepo::open(&repo_path).map_err(|e| e.to_string())?;

    // Pull
    let pull_result = repo.pull(&auth).map_err(|e| e.to_string())?;
    if matches!(pull_result, PullResult::Conflict) {
        return Err("remote has diverged — manual resolution required".to_string());
    }

    // Import remote changes
    let imported = state
        .db
        .with_conn(|conn| {
            sync::import_remote_servers(conn, &repo_path, &key, &team_id)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })
        .map_err(|e| e.to_string())?;

    // Detect remote deletions
    let deleted = state
        .db
        .with_conn(|conn| {
            sync::detect_remote_deletions(conn, &repo_path, &team_id)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })
        .map_err(|e| e.to_string())?;

    // Export local shared
    let exported = state
        .db
        .with_conn(|conn| {
            sync::export_local_shared(conn, &repo_path, &key, &username)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })
        .map_err(|e| e.to_string())?;

    // Commit + push if changes exist
    let status = repo.status().map_err(|e| e.to_string())?;
    if status.has_changes {
        let msg = format!("sync: {username} @ {}", now_rfc3339());
        repo.commit_and_push(&msg, &username, &auth)
            .map_err(|e| e.to_string())?;
    }

    // Update last sync time
    set_setting(&state, "team_last_sync", &now_rfc3339());

    let result = TeamSyncResult {
        imported,
        exported,
        conflicts: 0,
        deleted_remote: deleted,
    };

    crate::audit::log(&state.db, crate::audit::AuditEvent::TeamSync {
        imported, exported,
    });

    let _ = app.emit("team://synced", &result);

    Ok(result)
}

/// Leaves the team: remove self from members, cleanup local data.
#[tauri::command]
pub async fn team_leave(state: State<'_, AppState>) -> Result<(), String> {
    let repo_path = state.team_repo_path.read().await.clone()
        .ok_or("not in a team")?;

    // Try to remove self from team.json and push
    if let Ok(mut team) = sync::read_team_json(&repo_path) {
        let username = get_team_username(&state).unwrap_or_default();
        team.members.retain(|m| m.username != username);
        let _ = sync::write_team_json(&repo_path, &team);

        if let Ok(auth) = load_git_auth(&state) {
            if let Ok(repo) = TeamRepo::open(&repo_path) {
                let _ = repo.commit_and_push(
                    &format!("leave: {username}"),
                    &username,
                    &auth,
                );
            }
        }
    }

    let team_name = sync::read_team_json(&repo_path)
        .map(|t| t.name)
        .unwrap_or_default();
    crate::audit::log(&state.db, crate::audit::AuditEvent::TeamLeave { name: team_name });

    // Remove local repo
    let _ = std::fs::remove_dir_all(&repo_path);

    // Clear state
    *state.team_key.write().await = None;
    *state.team_repo_path.write().await = None;

    // Clear settings
    for key in &[
        "team_name", "team_repo_url", "team_repo_path", "team_username",
        "team_role", "team_last_sync", "team_git_auth_type", "team_git_ssh_key",
    ] {
        let _ = state.db.with_conn(|conn| {
            conn.execute("DELETE FROM settings WHERE key = ?1", rusqlite::params![key])
        });
    }

    // Mark shared servers as local-only (keep them but clear team_id)
    let _ = state.db.with_conn(|conn| {
        conn.execute(
            "UPDATE servers SET shared = 0, team_id = NULL WHERE team_id IS NOT NULL",
            [],
        )
    });

    Ok(())
}

/// Returns the current team status.
#[tauri::command]
pub async fn team_get_status(state: State<'_, AppState>) -> Result<TeamStatus, String> {
    let get = |key: &str| -> Option<String> {
        state
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    rusqlite::params![key],
                    |row| row.get(0),
                )
            })
            .ok()
    };

    let name = get("team_name");
    let joined = name.is_some();

    let has_pending = if let Some(ref path) = *state.team_repo_path.read().await {
        TeamRepo::open(path)
            .and_then(|r| r.status())
            .map(|s| s.has_changes)
            .unwrap_or(false)
    } else {
        false
    };

    let member_count = if let Some(ref path) = *state.team_repo_path.read().await {
        sync::read_team_json(path)
            .map(|t| t.members.len())
            .unwrap_or(0)
    } else {
        0
    };

    Ok(TeamStatus {
        joined,
        name,
        role: get("team_role"),
        member_count,
        last_sync: get("team_last_sync"),
        has_pending_changes: has_pending,
        repo_url: get("team_repo_url"),
    })
}

/// Lists team members from team.json.
#[tauri::command]
pub async fn team_list_members(state: State<'_, AppState>) -> Result<Vec<TeamMemberEntry>, String> {
    let repo_path = state.team_repo_path.read().await.clone()
        .ok_or("not in a team")?;
    let team = sync::read_team_json(&repo_path).map_err(|e| e.to_string())?;
    Ok(team.members)
}

/// Sets a member's role (admin only).
#[tauri::command]
pub async fn team_set_role(
    state: State<'_, AppState>,
    target_username: String,
    role: String,
) -> Result<(), String> {
    check_role(&state, "admin")?;

    let repo_path = state.team_repo_path.read().await.clone()
        .ok_or("not in a team")?;
    let mut team = sync::read_team_json(&repo_path).map_err(|e| e.to_string())?;

    let role_clone = role.clone();
    let member = team.members.iter_mut()
        .find(|m| m.username == target_username)
        .ok_or("member not found")?;
    member.role = role;

    sync::write_team_json(&repo_path, &team).map_err(|e| e.to_string())?;

    let auth = load_git_auth(&state)?;
    let username = get_team_username(&state)?;
    let repo = TeamRepo::open(&repo_path).map_err(|e| e.to_string())?;
    repo.commit_and_push(
        &format!("role: {target_username} → {}", team.members.iter().find(|m| m.username == target_username).map(|m| m.role.as_str()).unwrap_or("?")),
        &username,
        &auth,
    ).map_err(|e| e.to_string())?;

    crate::audit::log(&state.db, crate::audit::AuditEvent::TeamMemberRoleChange {
        target: target_username,
        role: role_clone,
    });

    Ok(())
}

/// Removes a member (admin only).
#[tauri::command]
pub async fn team_remove_member(
    state: State<'_, AppState>,
    target_username: String,
) -> Result<(), String> {
    check_role(&state, "admin")?;

    let repo_path = state.team_repo_path.read().await.clone()
        .ok_or("not in a team")?;
    let mut team = sync::read_team_json(&repo_path).map_err(|e| e.to_string())?;

    team.members.retain(|m| m.username != target_username);
    sync::write_team_json(&repo_path, &team).map_err(|e| e.to_string())?;

    let auth = load_git_auth(&state)?;
    let username = get_team_username(&state)?;
    let repo = TeamRepo::open(&repo_path).map_err(|e| e.to_string())?;
    repo.commit_and_push(
        &format!("remove: {target_username}"),
        &username,
        &auth,
    ).map_err(|e| e.to_string())?;

    crate::audit::log(&state.db, crate::audit::AuditEvent::TeamMemberRemove {
        target: target_username,
    });

    Ok(())
}

/// Verifies team passphrase and stores key in memory.
#[tauri::command]
pub async fn team_verify_passphrase(
    state: State<'_, AppState>,
    passphrase: String,
    remember: bool,
) -> Result<bool, String> {
    let repo_path = state.team_repo_path.read().await.clone()
        .ok_or("not in a team")?;
    let team = sync::read_team_json(&repo_path).map_err(|e| e.to_string())?;

    let salt_bytes = hex::decode(&team.salt).map_err(|_| "invalid salt")?;
    let salt: [u8; 16] = salt_bytes.try_into().map_err(|_| "invalid salt length")?;
    let key = derive_team_key(&passphrase, &salt).map_err(|e| e.to_string())?;

    if !verify_passphrase(&key, &team.verify) {
        return Ok(false);
    }

    *state.team_key.write().await = Some(key);

    if remember {
        let _ = crate::keychain::store("team_passphrase", &passphrase);
    }

    Ok(true)
}

/// Toggles a server's shared status.
#[tauri::command]
pub async fn team_toggle_share(
    state: State<'_, AppState>,
    server_id: String,
    shared: bool,
) -> Result<(), String> {
    check_role(&state, "member")?;

    let username = get_team_username(&state)?;
    let now = now_rfc3339();
    let team_name = state
        .db
        .with_conn(|conn| {
            conn.query_row("SELECT value FROM settings WHERE key='team_name'", [], |r| r.get::<_, String>(0))
        })
        .unwrap_or_default();
    let team_id = format!("team:{team_name}");

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE servers SET shared = ?1, team_id = ?2, shared_by = ?3, shared_at = ?4 WHERE id = ?5",
                rusqlite::params![shared as i32, if shared { Some(team_id.as_str()) } else { None::<&str> }, username, now, server_id],
            )
        })
        .map_err(|e| e.to_string())?;

    // Remove from repo if unsharing
    if !shared {
        if let Some(ref repo_path) = *state.team_repo_path.read().await {
            let file = repo_path.join("servers").join(format!("{server_id}.json"));
            let _ = std::fs::remove_file(&file);
        }
    }

    Ok(())
}

/// Rotates the team passphrase (admin only).
#[tauri::command]
pub async fn team_rotate_key(
    state: State<'_, AppState>,
    old_passphrase: String,
    new_passphrase: String,
) -> Result<(), String> {
    check_role(&state, "admin")?;

    if new_passphrase.len() < 8 {
        return Err("new passphrase must be at least 8 characters".to_string());
    }

    let repo_path = state.team_repo_path.read().await.clone()
        .ok_or("not in a team")?;
    let mut team = sync::read_team_json(&repo_path).map_err(|e| e.to_string())?;

    // Derive old key + verify
    let old_salt: [u8; 16] = hex::decode(&team.salt)
        .map_err(|_| "invalid salt")?
        .try_into()
        .map_err(|_| "invalid salt length")?;
    let old_key = derive_team_key(&old_passphrase, &old_salt).map_err(|e| e.to_string())?;

    if !verify_passphrase(&old_key, &team.verify) {
        return Err("current passphrase is incorrect".to_string());
    }

    // Generate new salt + key
    let new_salt = generate_team_salt().map_err(|e| e.to_string())?;
    let new_key = derive_team_key(&new_passphrase, &new_salt).map_err(|e| e.to_string())?;

    // Re-encrypt all server files
    let servers_dir = repo_path.join("servers");
    if servers_dir.exists() {
        for entry in std::fs::read_dir(&servers_dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let mut server: serde_json::Value =
                    serde_json::from_str(&content).map_err(|e| e.to_string())?;
                re_encrypt_json_field(&mut server, "password_enc", &old_key, &new_key)?;
                re_encrypt_json_field(&mut server, "passphrase_enc", &old_key, &new_key)?;
                let updated = serde_json::to_string_pretty(&server).map_err(|e| e.to_string())?;
                std::fs::write(&path, updated).map_err(|e| e.to_string())?;
            }
        }
    }

    // Update team.json
    team.salt = hex::encode(new_salt);
    team.verify = create_verify_token(&new_key).map_err(|e| e.to_string())?;
    sync::write_team_json(&repo_path, &team).map_err(|e| e.to_string())?;

    // Commit + push
    let auth = load_git_auth(&state)?;
    let username = get_team_username(&state)?;
    let repo = TeamRepo::open(&repo_path).map_err(|e| e.to_string())?;
    repo.commit_and_push("rotate team key", &username, &auth)
        .map_err(|e| e.to_string())?;

    // Update memory
    *state.team_key.write().await = Some(new_key);

    // Update keychain cache if remembered
    if crate::keychain::get("team_passphrase").is_ok() {
        let _ = crate::keychain::store("team_passphrase", &new_passphrase);
    }

    crate::audit::log(&state.db, crate::audit::AuditEvent::TeamKeyRotated);

    Ok(())
}

/// Re-encrypts a JSON field from old key to new key.
fn re_encrypt_json_field(
    obj: &mut serde_json::Value,
    field: &str,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> Result<(), String> {
    if let Some(enc) = obj.get(field).and_then(|v| v.as_str()) {
        if !enc.is_empty() {
            let new_enc = re_encrypt(old_key, new_key, enc).map_err(|e| e.to_string())?;
            obj[field] = serde_json::Value::String(new_enc);
        }
    }
    Ok(())
}
