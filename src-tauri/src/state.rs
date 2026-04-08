use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tokio::sync::{RwLock as TokioRwLock, oneshot};
use zeroize::Zeroizing;

use crate::keychain;
use crate::local_ai::LlamaServerState;
use crate::plugin::registry::PluginRegistry;
use crate::recording::recorder::RecorderRegistry;
use crate::sftp::session::SftpHandle;
use crate::ssh::forward::ForwardRegistry;
use crate::ssh::reverse_forward::{self, SharedReverseForwardRegistry};
use crate::ssh::session::SshSession;
use crate::storage::Database;

/// Entry in the proxy session pool (bastion/jump host).
/// Stores the authenticated SSH session and reference count for lifecycle management.
pub struct ProxyEntry {
    /// Authenticated SSH session to the bastion/proxy server (no shell channel opened)
    /// Used for direct-tcpip tunnel creation to inner targets
    pub session: Box<SshSession>,
    /// Number of inner sessions currently using this proxy
    pub ref_count: usize,
}

/// Global application state shared across all Tauri commands.
pub struct AppState {
    /// SQLCipher encrypted database connection.
    pub db: Database,
    /// Derived master encryption key — `None` when no master password is set.
    /// Wrapped in `Zeroizing` so the key is automatically zeroed on drop.
    pub master_key: RwLock<Option<Zeroizing<[u8; 32]>>>,
    /// Active SSH sessions keyed by session_id (tokio RwLock for async SFTP access).
    pub sessions: TokioRwLock<HashMap<String, SshSession>>,
    /// Active SFTP handles keyed by session_id (Arc-wrapped for task sharing).
    pub sftp_sessions: TokioRwLock<HashMap<String, Arc<SftpHandle>>>,
    /// Active port forwards.
    pub forwards: ForwardRegistry,
    /// Session recording manager.
    pub recorder: RecorderRegistry,
    /// Plugin registry.
    pub plugin_registry: RwLock<PluginRegistry>,
    /// Local AI (llama-server) process state.
    pub llama_server: TokioRwLock<LlamaServerState>,
    /// Active model downloads, keyed by model_id, with cancellation token.
    pub active_downloads: TokioRwLock<HashMap<String, oneshot::Sender<()>>>,
    /// Cached keychain verification state (true=verified, false=not verified, None=not checked yet)
    /// Only request user input on first check or when verification fails
    pub keychain_verified: RwLock<Option<bool>>,
    /// Proxy session pool for bastion/jump hosts, keyed by bastion server_id
    /// Multiple inner sessions can share the same bastion connection via reference counting
    /// Arc-wrapped so exit proxy tasks can hold a reference across async boundaries
    pub proxy_sessions: Arc<TokioRwLock<HashMap<String, ProxyEntry>>>,
    /// Reverse forward registry for Git Auto Sync notifications.
    pub reverse_forward_registry: SharedReverseForwardRegistry,
    /// Pending host key decisions: session_id → oneshot sender for user's trust decision.
    pub pending_host_key_decisions: TokioRwLock<HashMap<String, oneshot::Sender<bool>>>,
}

impl AppState {
    /// Creates a new AppState with an initialized database.
    pub fn new(master_password: Option<&str>) -> Result<Self, crate::storage::DbError> {
        let db = Database::open(master_password)?;
        let plugin_registry = PluginRegistry::new()
            .unwrap_or_else(|_| PluginRegistry::new_empty());

        let state = Self {
            db,
            master_key: RwLock::new(None),
            sessions: TokioRwLock::new(HashMap::new()),
            sftp_sessions: TokioRwLock::new(HashMap::new()),
            forwards: crate::ssh::forward::new_registry(),
            recorder: RecorderRegistry::new(),
            plugin_registry: RwLock::new(plugin_registry),
            llama_server: TokioRwLock::new(LlamaServerState::new()),
            active_downloads: TokioRwLock::new(HashMap::new()),
            keychain_verified: RwLock::new(None), // Will be checked once on startup
            proxy_sessions: Arc::new(TokioRwLock::new(HashMap::new())), // ProxyJump bastion pool
            reverse_forward_registry: reverse_forward::new_shared_registry(),
            pending_host_key_decisions: TokioRwLock::new(HashMap::new()),
        };

        // Initialize keychain (reads single store entry → at most 1 OS prompt)
        keychain::init();

        // Migrate legacy DB encrypted fields to keychain (one-time)
        state.migrate_to_keychain();

        Ok(state)
    }

    /// Migrates legacy `password_enc`/`api_key_enc` fields to the OS keychain.
    /// Runs once on startup; already-migrated rows (with keychain_id set) are skipped.
    fn migrate_to_keychain(&self) {
        if !keychain::is_available() {
            return;
        }

        // Migrate server passwords
        let _ = self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, password_enc, passphrase_enc FROM servers
                 WHERE password_keychain_id IS NULL AND password_enc IS NOT NULL"
            )?;
            let rows: Vec<(String, Option<Vec<u8>>, Option<Vec<u8>>)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .filter_map(|r| r.ok())
                .collect();

            for (id, pw_enc, pp_enc) in rows {
                if let Some(pw) = pw_enc {
                    let plain = String::from_utf8(pw).unwrap_or_default();
                    if !plain.is_empty() {
                        let kk = keychain::ssh_password_key(&id);
                        if keychain::store(&kk, &plain).is_ok() {
                            let _ = conn.execute(
                                "UPDATE servers SET password_keychain_id=?1, password_enc=NULL WHERE id=?2",
                                rusqlite::params![kk, id],
                            );
                        }
                    }
                }
                if let Some(pp) = pp_enc {
                    let plain = String::from_utf8(pp).unwrap_or_default();
                    if !plain.is_empty() {
                        let kk = keychain::ssh_passphrase_key(&id);
                        if keychain::store(&kk, &plain).is_ok() {
                            let _ = conn.execute(
                                "UPDATE servers SET passphrase_keychain_id=?1, passphrase_enc=NULL WHERE id=?2",
                                rusqlite::params![kk, id],
                            );
                        }
                    }
                }
            }
            Ok(())
        });

        // Migrate AI provider API keys
        let _ = self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, api_key_enc FROM ai_providers
                 WHERE api_key_keychain_id IS NULL AND api_key_enc IS NOT NULL"
            )?;
            let rows: Vec<(String, Vec<u8>)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();

            for (id, enc) in rows {
                let plain = String::from_utf8(enc).unwrap_or_default();
                if !plain.is_empty() {
                    let kk = keychain::ai_apikey_key(&id);
                    if keychain::store(&kk, &plain).is_ok() {
                        let _ = conn.execute(
                            "UPDATE ai_providers SET api_key_keychain_id=?1, api_key_enc=NULL WHERE id=?2",
                            rusqlite::params![kk, id],
                        );
                    }
                }
            }
            Ok(())
        });
    }

    /// Checks keychain verification state and detects system password changes.
    ///
    /// Returns:
    /// - `Ok(false)` if keychain verification is not available (normal for Linux with Secret Service)
    /// - `Ok(true)` if keychain is verified and accessible
    /// - `Err(...)` if keychain verification fails (system password may have changed)
    pub fn check_keychain_verification(&self) -> Result<bool, String> {
        // Check if we already verified in this app session
        if let Ok(cache) = self.keychain_verified.read() {
            if let Some(verified) = *cache {
                // Use cached result (silent, no prompt)
                return Ok(verified);
            }
        }

        // First time check - may prompt user
        eprintln!(">>> [KEYCHAIN] First check or cache miss, verifying...");
        match keychain::verify_accessible() {
            Ok(accessible) => {
                // Cache the result for this session
                if let Ok(mut cache) = self.keychain_verified.write() {
                    *cache = Some(accessible);
                }

                if accessible {
                    // Keychain token is accessible - mark as verified
                    let _ = self.db.with_conn(|conn| {
                        let now = time::OffsetDateTime::now_utc().to_string();
                        conn.execute(
                            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                            rusqlite::params!["keychain_verified_at", now, now],
                        )
                    });
                }
                Ok(accessible)
            }
            Err(e) => {
                // Keychain verification failed - likely system password changed
                eprintln!(">>> [KEYCHAIN] Verification failed: {}", e);
                // Cache the failure too (so we don't keep prompting)
                if let Ok(mut cache) = self.keychain_verified.write() {
                    *cache = Some(false);
                }
                Err(format!("Keychain verification failed: {}", e))
            }
        }
    }

    /// Records the current app version in settings for upgrade detection.
    pub fn update_app_version(&self, version: &str) {
        let _ = self.db.with_conn(|conn| {
            let now = time::OffsetDateTime::now_utc().to_string();
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                rusqlite::params!["app_version", version, now],
            )
        });
    }

    /// Checks if this is an application upgrade (for future use).
    #[allow(dead_code)]
    pub fn is_upgrade(&self, current_version: &str) -> Result<bool, String> {
        self.db
            .with_conn(|conn| {
                match conn.query_row(
                    "SELECT value FROM settings WHERE key = 'app_version'",
                    [],
                    |row| row.get::<_, String>(0),
                ) {
                    Ok(last_version) => Ok(last_version != current_version),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true), // First run, consider it an upgrade
                    Err(e) => Err(e),
                }
            })
            .map_err(|e| e.to_string())
    }
}
