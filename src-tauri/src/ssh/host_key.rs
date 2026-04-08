//! SSH Host Key verification using Trust On First Use (TOFU) model.
//!
//! Matches OpenSSH behavior:
//! - First connection: prompt user to trust the host key
//! - Subsequent connections: verify fingerprint matches stored record
//! - Key changed: warn user of potential MITM attack

use russh::keys::{HashAlg, PublicKey};

use crate::storage::Database;

/// Result of host key verification.
#[derive(Debug, Clone, serde::Serialize)]
pub enum HostKeyVerifyResult {
    /// Fingerprint matches stored record — connection is trusted.
    Trusted,
    /// First time connecting to this host — user must confirm trust.
    NewHost {
        key_type: String,
        fingerprint: String,
    },
    /// Host key has changed — possible MITM attack.
    KeyChanged {
        key_type: String,
        old_fingerprint: String,
        new_fingerprint: String,
    },
}

/// Extracts the key type string from a public key.
fn key_type_str(key: &PublicKey) -> String {
    key.algorithm().as_str().to_string()
}

/// Computes the SHA-256 fingerprint of a public key.
/// Returns the fingerprint in the standard `SHA256:base64` format.
fn compute_fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// Verifies a server's public key against the known_hosts database.
///
/// Returns the verification result indicating whether the host is trusted,
/// new, or has a changed key.
pub fn verify_host_key(
    db: &Database,
    host: &str,
    port: u16,
    server_public_key: &PublicKey,
) -> HostKeyVerifyResult {
    let key_type = key_type_str(server_public_key);
    let fingerprint = compute_fingerprint(server_public_key);

    let stored = db.with_conn(|conn| {
        conn.query_row(
            "SELECT fingerprint FROM known_hosts WHERE host = ?1 AND port = ?2 AND key_type = ?3",
            rusqlite::params![host, port as i32, key_type],
            |row| row.get::<_, String>(0),
        )
    });

    match stored {
        Ok(old_fp) => {
            if old_fp == fingerprint {
                // Update last_seen timestamp
                let _ = db.with_conn(|conn| {
                    let now = time::OffsetDateTime::now_utc().to_string();
                    conn.execute(
                        "UPDATE known_hosts SET last_seen = ?1 WHERE host = ?2 AND port = ?3 AND key_type = ?4",
                        rusqlite::params![now, host, port as i32, key_type],
                    )
                });
                HostKeyVerifyResult::Trusted
            } else {
                HostKeyVerifyResult::KeyChanged {
                    key_type,
                    old_fingerprint: old_fp,
                    new_fingerprint: fingerprint,
                }
            }
        }
        Err(_) => {
            // No record found — new host
            HostKeyVerifyResult::NewHost {
                key_type,
                fingerprint,
            }
        }
    }
}

/// Stores a trusted host key in the known_hosts database.
pub fn trust_host_key(
    db: &Database,
    host: &str,
    port: u16,
    server_public_key: &PublicKey,
) -> Result<(), String> {
    let key_type = key_type_str(server_public_key);
    let fingerprint = compute_fingerprint(server_public_key);
    let now = time::OffsetDateTime::now_utc().to_string();

    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, COALESCE((SELECT first_seen FROM known_hosts WHERE host=?1 AND port=?2 AND key_type=?3), ?5), ?5)",
            rusqlite::params![host, port as i32, key_type, fingerprint, now],
        )
    })
    .map_err(|e| format!("Failed to store host key: {}", e))?;
    Ok(())
}

/// Removes a host key entry (used before storing a new key after user confirms KeyChanged).
pub fn remove_host_key(
    db: &Database,
    host: &str,
    port: u16,
) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute(
            "DELETE FROM known_hosts WHERE host = ?1 AND port = ?2",
            rusqlite::params![host, port as i32],
        )
    })
    .map_err(|e| format!("Failed to remove host key: {}", e))?;
    Ok(())
}
