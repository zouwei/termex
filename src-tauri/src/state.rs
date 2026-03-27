use std::collections::HashMap;
use std::sync::RwLock;

use tokio::sync::RwLock as TokioRwLock;

use crate::plugin::registry::PluginRegistry;
use crate::recording::recorder::RecorderRegistry;
use crate::sftp::session::SftpHandle;
use crate::ssh::forward::ForwardRegistry;
use crate::ssh::session::SshSession;
use crate::storage::Database;

/// Global application state shared across all Tauri commands.
pub struct AppState {
    /// SQLCipher encrypted database connection.
    pub db: Database,
    /// Derived master encryption key — `None` when no master password is set.
    pub master_key: RwLock<Option<[u8; 32]>>,
    /// Active SSH sessions keyed by session_id (tokio RwLock for async SFTP access).
    pub sessions: TokioRwLock<HashMap<String, SshSession>>,
    /// Active SFTP handles keyed by session_id.
    pub sftp_sessions: TokioRwLock<HashMap<String, SftpHandle>>,
    /// Active port forwards.
    pub forwards: ForwardRegistry,
    /// Session recording manager.
    pub recorder: RecorderRegistry,
    /// Plugin registry.
    pub plugin_registry: RwLock<PluginRegistry>,
}

impl AppState {
    /// Creates a new AppState with an initialized database.
    pub fn new(master_password: Option<&str>) -> Result<Self, crate::storage::DbError> {
        let db = Database::open(master_password)?;
        let plugin_registry = PluginRegistry::new()
            .unwrap_or_else(|_| PluginRegistry::new_empty());

        Ok(Self {
            db,
            master_key: RwLock::new(None),
            sessions: TokioRwLock::new(HashMap::new()),
            sftp_sessions: TokioRwLock::new(HashMap::new()),
            forwards: crate::ssh::forward::new_registry(),
            recorder: RecorderRegistry::new(),
            plugin_registry: RwLock::new(plugin_registry),
        })
    }
}
