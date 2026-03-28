use std::collections::HashMap;
use std::sync::RwLock;

use tokio::sync::RwLock as TokioRwLock;

use crate::keychain;
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

        let state = Self {
            db,
            master_key: RwLock::new(None),
            sessions: TokioRwLock::new(HashMap::new()),
            sftp_sessions: TokioRwLock::new(HashMap::new()),
            forwards: crate::ssh::forward::new_registry(),
            recorder: RecorderRegistry::new(),
            plugin_registry: RwLock::new(plugin_registry),
        };

        // Migrate legacy encrypted credentials to OS keychain
        state.migrate_to_keychain();

        // Preload all keychain credentials into in-memory cache
        // so subsequent access never triggers OS password prompts
        state.preload_credentials();

        Ok(state)
    }

    /// Preloads all known keychain credentials into the in-memory cache.
    /// This batch-reads from the OS keychain once on startup so that all
    /// subsequent `keychain::get()` calls are served from memory.
    fn preload_credentials(&self) {
        if !keychain::is_available() {
            return;
        }

        let mut keys: Vec<String> = Vec::new();

        // Collect server credential keys
        let _ = self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id FROM servers WHERE password_keychain_id IS NOT NULL
                 OR passphrase_keychain_id IS NOT NULL"
            )?;
            let ids: Vec<String> = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            for id in ids {
                keys.push(keychain::ssh_password_key(&id));
                keys.push(keychain::ssh_passphrase_key(&id));
            }
            Ok(())
        });

        // Collect AI provider API key keys
        let _ = self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id FROM ai_providers WHERE api_key_keychain_id IS NOT NULL"
            )?;
            let ids: Vec<String> = stmt
                .query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            for id in ids {
                keys.push(keychain::ai_apikey_key(&id));
            }
            Ok(())
        });

        if !keys.is_empty() {
            keychain::preload(&keys);
        }
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
}
