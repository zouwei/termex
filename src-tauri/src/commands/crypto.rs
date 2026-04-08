use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use tauri::State;

use crate::crypto::{aes, kdf, CryptoError};
use crate::state::AppState;

/// Failed password attempt counter (resets on app restart).
static FAILED_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
/// Timestamp (epoch seconds) when the lockout expires.
static LOCKOUT_UNTIL: AtomicU64 = AtomicU64::new(0);

/// Checks whether a master password has been configured.
#[tauri::command]
pub fn master_password_exists(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT 1 FROM settings WHERE key = 'master_salt'")?;
            Ok(stmt.exists([])?)
        })
        .map_err(|e| e.to_string())
}

/// Sets the master password for the first time.
/// Derives a key via Argon2id and stores the salt + a verification token.
#[tauri::command]
pub fn master_password_set(state: State<'_, AppState>, password: String) -> Result<(), String> {
    // Reject if already set
    if master_password_exists(state.clone())? {
        return Err("Master password already set. Use change instead.".into());
    }

    // Validate password strength
    crate::crypto::password_policy::validate_master_password(&password)
        .map_err(|feedback| feedback.join("; "))?;

    let (key, salt) = kdf::derive_key_new(&password).map_err(|e| e.to_string())?;

    // Encrypt a known token to allow future password verification
    let verify_token = aes::encrypt(&key, b"TERMEX_VERIFY").map_err(|e| e.to_string())?;
    let salt_hex = hex_encode(&salt);
    let token_hex = hex_encode(&verify_token);
    let now = time::OffsetDateTime::now_utc().to_string();

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('master_salt', ?1, ?2)",
                rusqlite::params![salt_hex, now],
            )?;
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('master_verify', ?1, ?2)",
                rusqlite::params![token_hex, now],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    // Store derived key in memory
    let mut mk = state.master_key.write().expect("master_key lock poisoned");
    *mk = Some(key);

    crate::audit::log(&state.db, crate::audit::AuditEvent::MasterPasswordSet);

    Ok(())
}

/// Verifies the master password and loads the derived key into memory.
#[tauri::command]
pub fn master_password_verify(
    state: State<'_, AppState>,
    password: String,
) -> Result<bool, String> {
    // Check if currently locked out
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let lockout = LOCKOUT_UNTIL.load(Ordering::Relaxed);
    if now < lockout {
        let remaining = lockout - now;
        return Err(format!("Too many failed attempts. Try again in {} seconds.", remaining));
    }

    let (salt, verify_token) = load_salt_and_token(&state)?;
    let key = kdf::derive_key(&password, &salt).map_err(|e| e.to_string())?;

    match aes::decrypt(&key, &verify_token) {
        Ok(plaintext) if plaintext == b"TERMEX_VERIFY" => {
            FAILED_ATTEMPTS.store(0, Ordering::Relaxed);
            let mut mk = state.master_key.write().expect("master_key lock poisoned");
            *mk = Some(key);
            crate::audit::log(&state.db, crate::audit::AuditEvent::MasterPasswordVerified);
            Ok(true)
        }
        _ => {
            let attempts = FAILED_ATTEMPTS.fetch_add(1, Ordering::Relaxed) + 1;
            if attempts >= 10 {
                // Lock for 5 minutes
                LOCKOUT_UNTIL.store(now + 300, Ordering::Relaxed);
            } else if attempts >= 5 {
                // Lock for 30 seconds
                LOCKOUT_UNTIL.store(now + 30, Ordering::Relaxed);
            }
            crate::audit::log(&state.db, crate::audit::AuditEvent::MasterPasswordFailed);
            Ok(false)
        }
    }
}

/// Changes the master password: re-encrypts all encrypted fields.
#[tauri::command]
pub fn master_password_change(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    // Verify old password
    let (old_salt, old_verify) = load_salt_and_token(&state)?;
    let old_key = kdf::derive_key(&old_password, &old_salt).map_err(|e| e.to_string())?;

    match aes::decrypt(&old_key, &old_verify) {
        Ok(p) if p == b"TERMEX_VERIFY" => {}
        _ => return Err(CryptoError::WrongPassword.to_string()),
    }

    // Validate new password strength
    crate::crypto::password_policy::validate_master_password(&new_password)
        .map_err(|feedback| feedback.join("; "))?;

    // Derive new key
    let (new_key, new_salt) = kdf::derive_key_new(&new_password).map_err(|e| e.to_string())?;

    // Re-encrypt all _enc fields
    re_encrypt_all(&state, &old_key, &new_key)?;

    // Update salt and verify token
    let new_verify = aes::encrypt(&new_key, b"TERMEX_VERIFY").map_err(|e| e.to_string())?;
    let now = time::OffsetDateTime::now_utc().to_string();

    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE settings SET value = ?1, updated_at = ?2 WHERE key = 'master_salt'",
                rusqlite::params![hex_encode(&new_salt), now],
            )?;
            conn.execute(
                "UPDATE settings SET value = ?1, updated_at = ?2 WHERE key = 'master_verify'",
                rusqlite::params![hex_encode(&new_verify), now],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    // Update in-memory key
    let mut mk = state.master_key.write().expect("master_key lock poisoned");
    *mk = Some(new_key);

    crate::audit::log(&state.db, crate::audit::AuditEvent::MasterPasswordChanged);

    Ok(())
}

/// Clears the master key from memory (lock the app).
/// Setting to `None` drops the `Zeroizing<[u8; 32]>`, which zeroes memory automatically.
#[tauri::command]
pub fn master_password_lock(state: State<'_, AppState>) -> Result<(), String> {
    let mut mk = state.master_key.write().expect("master_key lock poisoned");
    *mk = None;
    drop(mk);
    crate::audit::log(&state.db, crate::audit::AuditEvent::MasterPasswordLocked);
    Ok(())
}

/// Verifies keychain access and handles system password changes.
///
/// Called by frontend when the app needs to verify that credentials are still accessible
/// (e.g., after system password change detection). Returns `Ok(())` if verification succeeds.
#[tauri::command]
pub fn keychain_verify(state: State<'_, AppState>) -> Result<(), String> {
    state.check_keychain_verification()?;
    Ok(())
}

/// Checks password strength and returns score + feedback.
#[tauri::command]
pub fn check_password_strength(password: String) -> serde_json::Value {
    let strength = crate::crypto::password_policy::check_strength(&password);
    serde_json::json!({
        "score": strength.score,
        "feedback": strength.feedback,
    })
}

// ── Internal helpers ───────────────────────────────────────────

/// Loads the stored salt and verification token from the settings table.
fn load_salt_and_token(state: &AppState) -> Result<([u8; 16], Vec<u8>), String> {
    state
        .db
        .with_conn(|conn| {
            let salt_hex: String = conn.query_row(
                "SELECT value FROM settings WHERE key = 'master_salt'",
                [],
                |row| row.get(0),
            )?;
            let token_hex: String = conn.query_row(
                "SELECT value FROM settings WHERE key = 'master_verify'",
                [],
                |row| row.get(0),
            )?;

            let salt_bytes = hex_decode(&salt_hex);
            let mut salt = [0u8; 16];
            if salt_bytes.len() != 16 {
                return Err(rusqlite::Error::InvalidParameterName(
                    "corrupt master salt".into(),
                ));
            }
            salt.copy_from_slice(&salt_bytes);

            let token = hex_decode(&token_hex);
            Ok((salt, token))
        })
        .map_err(|e| e.to_string())
}

/// Re-encrypts all encrypted fields from old_key to new_key.
fn re_encrypt_all(state: &AppState, old_key: &[u8; 32], new_key: &[u8; 32]) -> Result<(), String> {
    // Re-encrypt server passwords and passphrases
    re_encrypt_table_field(state, "servers", "id", "password_enc", old_key, new_key)?;
    re_encrypt_table_field(state, "servers", "id", "passphrase_enc", old_key, new_key)?;
    // Re-encrypt SSH key passphrases
    re_encrypt_table_field(state, "ssh_keys", "id", "passphrase_enc", old_key, new_key)?;
    // Re-encrypt AI provider API keys
    re_encrypt_table_field(state, "ai_providers", "id", "api_key_enc", old_key, new_key)?;

    Ok(())
}

/// Re-encrypts a single BLOB column across all rows of a table.
fn re_encrypt_table_field(
    state: &AppState,
    table: &str,
    pk: &str,
    column: &str,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> Result<(), String> {
    // Whitelist table and column names to prevent SQL injection
    let allowed_tables = ["servers", "ssh_keys", "ai_providers"];
    let allowed_columns = ["password_enc", "passphrase_enc", "api_key_enc"];
    if !allowed_tables.contains(&table) || !allowed_columns.contains(&column) {
        return Err(format!("invalid table/column: {table}.{column}"));
    }

    state
        .db
        .with_conn(|conn| {
            let query = format!("SELECT {pk}, {column} FROM {table} WHERE {column} IS NOT NULL");
            let mut stmt = conn.prepare(&query)?;
            let rows: Vec<(String, Vec<u8>)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();

            let update_sql = format!("UPDATE {table} SET {column} = ?1 WHERE {pk} = ?2");
            for (id, encrypted) in rows {
                let plaintext = aes::decrypt(old_key, &encrypted)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                let re_encrypted = aes::encrypt(new_key, &plaintext)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                conn.execute(&update_sql, rusqlite::params![re_encrypted, id])?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())
}

/// Hex encode bytes.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex decode a string.
fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
