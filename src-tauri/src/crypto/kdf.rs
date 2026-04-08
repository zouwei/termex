use argon2::{Argon2, Algorithm, Params, Version};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

use super::CryptoError;

/// Salt length in bytes.
const SALT_LEN: usize = 16;

/// Argon2id parameters matching the security spec:
/// m=64MB, t=3 iterations, p=4 parallelism.
fn argon2_params() -> Params {
    Params::new(64 * 1024, 3, 4, Some(32)).expect("valid Argon2 params")
}

/// Generates a cryptographically secure random salt.
pub fn generate_salt() -> Result<[u8; SALT_LEN], CryptoError> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt)
        .map_err(|_| CryptoError::RngFailed)?;
    Ok(salt)
}

/// Derives a 32-byte master key from a password and salt using Argon2id.
///
/// Returns a `Zeroizing<[u8; 32]>` that automatically zeroes the key on drop.
pub fn derive_key(password: &str, salt: &[u8; SALT_LEN]) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params());

    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut *key)
        .map_err(|_| CryptoError::KdfFailed)?;

    Ok(key)
}

/// Derives a key and returns both the key and a newly generated salt.
/// Used when setting a master password for the first time.
pub fn derive_key_new(password: &str) -> Result<(Zeroizing<[u8; 32]>, [u8; SALT_LEN]), CryptoError> {
    let salt = generate_salt()?;
    let key = derive_key(password, &salt)?;
    Ok((key, salt))
}