use serde::{Deserialize, Serialize};

// ============================================================
// Server Group
// ============================================================

/// A group for organizing servers in the sidebar tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerGroup {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub parent_id: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================
// Server
// ============================================================

/// Authentication type for an SSH connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    Password,
    Key,
}

impl AuthType {
    /// Converts to database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Key => "key",
        }
    }

    /// Parses from database string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "password" => Some(Self::Password),
            "key" => Some(Self::Key),
            _ => None,
        }
    }
}

/// An SSH server connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub auth_type: AuthType,
    /// Encrypted password bytes — never sent to frontend.
    #[serde(skip_serializing)]
    pub password_enc: Option<Vec<u8>>,
    pub key_path: Option<String>,
    /// Encrypted key passphrase bytes — never sent to frontend.
    #[serde(skip_serializing)]
    pub passphrase_enc: Option<Vec<u8>>,
    pub group_id: Option<String>,
    pub sort_order: i32,
    pub proxy_id: Option<String>,
    pub network_proxy_id: Option<String>,
    pub startup_cmd: Option<String>,
    pub encoding: String,
    pub tags: Vec<String>,
    /// tmux mode: "disabled" | "auto" | "always"
    pub tmux_mode: String,
    /// Action on tab close: "detach" | "kill"
    pub tmux_close_action: String,
    /// Whether Git Auto Sync is enabled for this server.
    pub git_sync_enabled: bool,
    /// Git sync mode: "notify" | "auto_pull"
    pub git_sync_mode: String,
    /// Local git repository path for auto-pull.
    pub git_sync_local_path: Option<String>,
    /// Remote git repository path.
    pub git_sync_remote_path: Option<String>,
    pub last_connected: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================
// Network Proxy
// ============================================================

/// A network proxy configuration (SOCKS5 / SOCKS4 / HTTP CONNECT / ProxyCommand).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub id: String,
    pub name: String,
    pub proxy_type: String,
    pub host: String,
    pub port: i32,
    pub username: Option<String>,
    /// Encrypted password bytes — never sent to frontend.
    #[serde(skip_serializing)]
    pub password_enc: Option<Vec<u8>>,
    pub password_keychain_id: Option<String>,
    pub tls_enabled: bool,
    pub tls_verify: bool,
    pub ca_cert_path: Option<String>,
    pub client_cert_path: Option<String>,
    pub client_key_path: Option<String>,
    /// ProxyCommand string (only for `command` type proxies).
    pub command: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================
// SSH Key
// ============================================================

/// A managed SSH key entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKey {
    pub id: String,
    pub name: String,
    pub key_type: String,
    pub bits: Option<i32>,
    pub file_path: String,
    pub public_key: Option<String>,
    pub has_passphrase: bool,
    #[serde(skip_serializing)]
    pub passphrase_enc: Option<Vec<u8>>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================
// Port Forward
// ============================================================

/// Port forward direction type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ForwardType {
    Local,
    Remote,
    Dynamic,
}

impl ForwardType {
    /// Converts to database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::Dynamic => "dynamic",
        }
    }

    /// Parses from database string representation.
    pub fn from_str(s: &str) -> Self {
        match s {
            "remote" => Self::Remote,
            "dynamic" => Self::Dynamic,
            _ => Self::Local,
        }
    }
}

/// A port forwarding rule associated with a server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortForward {
    pub id: String,
    pub server_id: String,
    pub forward_type: ForwardType,
    pub local_host: String,
    pub local_port: i32,
    pub remote_host: Option<String>,
    pub remote_port: Option<i32>,
    pub auto_start: bool,
    pub enabled: bool,
    pub created_at: String,
}

// ============================================================
// AI Provider
// ============================================================

/// AI provider backend type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Claude,
    Openai,
    Gemini,
    Deepseek,
    Ollama,
    Grok,
    Mistral,
    Glm,
    Minimax,
    Doubao,
    Local,
    Custom,
}

impl ProviderType {
    /// Converts to database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
            Self::Deepseek => "deepseek",
            Self::Ollama => "ollama",
            Self::Grok => "grok",
            Self::Mistral => "mistral",
            Self::Glm => "glm",
            Self::Minimax => "minimax",
            Self::Doubao => "doubao",
            Self::Local => "local",
            Self::Custom => "custom",
        }
    }
}

/// An AI provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    pub provider_type: ProviderType,
    #[serde(skip_serializing)]
    pub api_key_enc: Option<Vec<u8>>,
    pub api_base_url: Option<String>,
    pub model: String,
    pub max_tokens: i32,
    pub temperature: f64,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================
// Known Host
// ============================================================

/// A known SSH host fingerprint entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHost {
    pub host: String,
    pub port: i32,
    pub key_type: String,
    pub fingerprint: String,
    pub first_seen: String,
    pub last_seen: String,
}
