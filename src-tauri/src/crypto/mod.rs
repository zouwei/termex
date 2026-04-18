pub mod aes;
pub mod kdf;
pub mod password_policy;

#[cfg(feature = "sentinel")]
pub mod token_verify;
#[cfg(feature = "sentinel")]
pub mod key_spectrum;
#[cfg(feature = "sentinel")]
pub mod traits;
#[cfg(feature = "sentinel")]
pub mod entropy_pool;

/// Cryptography error types.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("random number generation failed")]
    RngFailed,

    #[error("invalid encryption key")]
    InvalidKey,

    #[error("encryption failed")]
    EncryptFailed,

    #[error("decryption failed — wrong key or corrupted data")]
    DecryptFailed,

    #[error("encrypted data too short to contain nonce and tag")]
    DataTooShort,

    #[error("key derivation failed")]
    KdfFailed,

    #[error("master password not set")]
    NoMasterPassword,

    #[error("master password is incorrect")]
    WrongPassword,

    #[error("database error: {0}")]
    Database(#[from] crate::storage::DbError),
}
