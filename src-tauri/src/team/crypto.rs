use base64::Engine;
use zeroize::Zeroizing;

use crate::crypto::{aes, kdf, CryptoError};

/// Salt length for team key derivation.
pub const TEAM_SALT_LEN: usize = 16;

/// Fixed verification plaintext used to validate team passphrase.
pub const VERIFY_PLAINTEXT: &str = "TERMEX_TEAM_VERIFY";

/// Error types for team crypto operations.
#[derive(Debug, thiserror::Error)]
pub enum TeamCryptoError {
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("base64 decode error: {0}")]
    Base64Decode(String),
    #[error("UTF-8 decode error")]
    Utf8,
    #[error("invalid encrypted data")]
    InvalidData,
}

/// Derives a 32-byte team key from passphrase + salt using Argon2id.
///
/// Same parameters as personal master key (m=64MB, t=3, p=4).
/// All team members with the same passphrase + salt get the same key.
pub fn derive_team_key(
    passphrase: &str,
    salt: &[u8; TEAM_SALT_LEN],
) -> Result<Zeroizing<[u8; 32]>, TeamCryptoError> {
    kdf::derive_key(passphrase, salt).map_err(TeamCryptoError::Crypto)
}

/// Generates a random 16-byte salt for team key derivation.
pub fn generate_team_salt() -> Result<[u8; TEAM_SALT_LEN], TeamCryptoError> {
    kdf::generate_salt().map_err(TeamCryptoError::Crypto)
}

/// Encrypts a string with the team key, returning base64-encoded ciphertext.
///
/// Format: base64(nonce[12] + ciphertext + tag[16])
pub fn team_encrypt(key: &[u8; 32], plaintext: &str) -> Result<String, TeamCryptoError> {
    let encrypted = aes::encrypt(key, plaintext.as_bytes())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&encrypted))
}

/// Decrypts a base64-encoded ciphertext with the team key.
pub fn team_decrypt(key: &[u8; 32], ciphertext_b64: &str) -> Result<String, TeamCryptoError> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(ciphertext_b64)
        .map_err(|e| TeamCryptoError::Base64Decode(e.to_string()))?;

    if data.len() < 12 + 16 {
        return Err(TeamCryptoError::InvalidData);
    }

    let plaintext_bytes = aes::decrypt(key, &data)?;
    String::from_utf8(plaintext_bytes).map_err(|_| TeamCryptoError::Utf8)
}

/// Creates a verification token for the team passphrase.
///
/// Stored in team.json `_verify` field. Used to check passphrase correctness
/// without needing to decrypt actual server credentials.
pub fn create_verify_token(key: &[u8; 32]) -> Result<String, TeamCryptoError> {
    team_encrypt(key, VERIFY_PLAINTEXT)
}

/// Validates a passphrase by attempting to decrypt the verification token.
pub fn verify_passphrase(key: &[u8; 32], verify_token: &str) -> bool {
    match team_decrypt(key, verify_token) {
        Ok(plaintext) => plaintext == VERIFY_PLAINTEXT,
        Err(_) => false,
    }
}

/// Re-encrypts a field value from old key to new key.
///
/// Used during passphrase rotation.
pub fn re_encrypt(
    old_key: &[u8; 32],
    new_key: &[u8; 32],
    ciphertext_b64: &str,
) -> Result<String, TeamCryptoError> {
    let plaintext = team_decrypt(old_key, ciphertext_b64)?;
    team_encrypt(new_key, &plaintext)
}
