use ring::aead::{self, Aad, BoundKey, Nonce, NonceSequence, NONCE_LEN};
use ring::error::Unspecified;
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

use super::CryptoError;

/// AES-256-GCM authentication tag length.
const TAG_LEN: usize = 16;

/// Single-use nonce generator for AES-256-GCM.
struct OneNonceSequence(Option<Nonce>);

impl OneNonceSequence {
    fn new(nonce_bytes: [u8; NONCE_LEN]) -> Self {
        Self(Some(Nonce::assume_unique_for_key(nonce_bytes)))
    }
}

impl NonceSequence for OneNonceSequence {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        self.0.take().ok_or(Unspecified)
    }
}

/// Encrypts plaintext using AES-256-GCM with the given 32-byte key.
///
/// Returns bytes in the format: `[nonce (12B) | ciphertext | tag (16B)]`.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let rng = SystemRandom::new();

    // Generate random 12-byte nonce
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| CryptoError::RngFailed)?;

    let unbound_key =
        aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| CryptoError::InvalidKey)?;

    let nonce_seq = OneNonceSequence::new(nonce_bytes);
    let mut sealing_key = aead::SealingKey::new(unbound_key, nonce_seq);

    // Encrypt in-place: buffer = plaintext + space for tag
    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut in_out)
        .map_err(|_| CryptoError::EncryptFailed)?;

    // Prepend nonce: [nonce | ciphertext | tag]
    let mut result = Vec::with_capacity(NONCE_LEN + in_out.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&in_out);

    Ok(result)
}

/// Decrypts data produced by [`encrypt`].
///
/// Input format: `[nonce (12B) | ciphertext | tag (16B)]`.
pub fn decrypt(key: &[u8; 32], encrypted: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if encrypted.len() < NONCE_LEN + TAG_LEN {
        return Err(CryptoError::DataTooShort);
    }

    // Split nonce from ciphertext+tag
    let (nonce_bytes, ciphertext_with_tag) = encrypted.split_at(NONCE_LEN);
    let mut nonce_arr = [0u8; NONCE_LEN];
    nonce_arr.copy_from_slice(nonce_bytes);

    let unbound_key =
        aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| CryptoError::InvalidKey)?;

    let nonce_seq = OneNonceSequence::new(nonce_arr);
    let mut opening_key = aead::OpeningKey::new(unbound_key, nonce_seq);

    // Decrypt in-place (wrap in Zeroizing so intermediate plaintext is zeroed on drop)
    let mut in_out = Zeroizing::new(ciphertext_with_tag.to_vec());
    let plaintext = opening_key
        .open_in_place(Aad::empty(), &mut *in_out)
        .map_err(|_| CryptoError::DecryptFailed)?;

    Ok(plaintext.to_vec())
}