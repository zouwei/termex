use serde::{Deserialize, Serialize};

/// Team metadata stored in team.json at the repo root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamJson {
    pub version: u32,
    pub name: String,
    /// Hex-encoded salt for key derivation.
    #[serde(rename = "_salt")]
    pub salt: String,
    /// Base64-encoded verification token (encrypted "TERMEX_TEAM_VERIFY").
    #[serde(rename = "_verify")]
    pub verify: String,
    pub members: Vec<TeamMemberEntry>,
    #[serde(default)]
    pub settings: TeamSettings,
}

/// A member entry in team.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMemberEntry {
    pub username: String,
    pub role: String,
    pub joined_at: String,
    pub device_id: String,
}

/// Team-level settings in team.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TeamSettings {
    pub allow_member_push: bool,
    pub require_admin_approve: bool,
}

impl Default for TeamSettings {
    fn default() -> Self {
        Self {
            allow_member_push: true,
            require_admin_approve: false,
        }
    }
}

/// Server configuration shared via Git repo (servers/{id}.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedServerConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    /// AES-256-GCM encrypted password (base64), or null.
    pub password_enc: Option<String>,
    /// AES-256-GCM encrypted passphrase (base64), or null.
    pub passphrase_enc: Option<String>,
    pub group_id: Option<String>,
    #[serde(default)]
    pub tags: String,
    pub startup_cmd: Option<String>,
    #[serde(default = "default_encoding")]
    pub encoding: String,
    #[serde(default)]
    pub auto_record: bool,
    pub shared_by: String,
    pub shared_at: String,
    pub updated_at: String,
}

fn default_encoding() -> String {
    "UTF-8".to_string()
}

/// Snippet shared via Git repo (snippets/{id}.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSnippet {
    pub id: String,
    pub title: String,
    pub command: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub folder_id: Option<String>,
    pub shared_by: String,
    pub shared_at: String,
    pub updated_at: String,
}

/// Snippet folder list (snippets/folders.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFolders {
    pub folders: Vec<SharedFolder>,
}

/// A snippet folder entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFolder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub sort_order: i32,
}

/// Group hierarchy (groups/groups.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedGroups {
    pub groups: Vec<SharedGroup>,
}

/// A server group entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedGroup {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
}

/// Proxy list (proxies/proxies.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedProxies {
    pub proxies: Vec<SharedProxy>,
}

/// A proxy configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedProxy {
    pub id: String,
    pub name: String,
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    /// AES-256-GCM encrypted username (base64), or null.
    pub username_enc: Option<String>,
    /// AES-256-GCM encrypted password (base64), or null.
    pub password_enc: Option<String>,
    pub shared_by: String,
    pub updated_at: String,
}

// ── Tauri command return types ──

/// Returned by team_create / team_join.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamInfo {
    pub name: String,
    pub repo_url: String,
    pub role: String,
    pub member_count: usize,
    pub created_at: String,
}

/// Returned by team_get_status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStatus {
    pub joined: bool,
    pub name: Option<String>,
    pub role: Option<String>,
    pub member_count: usize,
    pub last_sync: Option<String>,
    pub has_pending_changes: bool,
    pub repo_url: Option<String>,
}

/// Returned by team_sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSyncResult {
    pub imported: usize,
    pub exported: usize,
    pub conflicts: usize,
    pub deleted_remote: usize,
}

/// Git authentication configuration from frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAuthConfig {
    pub auth_type: String,
    pub ssh_key_path: Option<String>,
    pub ssh_passphrase: Option<String>,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}
