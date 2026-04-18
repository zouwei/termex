//! Entropy mixing pool for combining multiple cryptographic entropy sources.
//!
//! The pool collects entropy from all registered [`EntropySource`] instances
//! and mixes them using quality-weighted XOR folding. The mixed output seeds
//! key generation, nonce derivation, and session token creation.
//!
//! Architecture:
//! ```text
//! SystemEntropy ──┐
//!                 ├──> EntropyPool::harvest_mixed() ──> raw key material
//! (future HW) ───┘            |
//!                    KeyValidator::validate()
//!                             |
//!                       accepted / rejected
//! ```
//!
//! The pool enforces a minimum quality threshold: sources with `quality() < 0.1`
//! are excluded from the mix to prevent entropy dilution attacks.
//!
//! Reference: NIST SP 800-90C §7 "Entropy Source Health Testing"
//!
//! SECURITY: The pool MUST contain at least one full-entropy source (quality = 1.0)
//! before `generate_validated_key()` is called. Using only degraded sources
//! produces key material that may not meet the 256-bit security level.

use super::traits::{EntropySource, KeyValidator};

/// Minimum source quality to include in entropy mixing.
/// Sources below this threshold are ignored to prevent dilution.
const MIN_SOURCE_QUALITY: f64 = 0.1;

/// Maximum key generation attempts before returning failure.
/// Each attempt uses a different seed derivation to explore the key space.
const MAX_KEYGEN_ATTEMPTS: u32 = 100;

/// A pool that mixes entropy from multiple sources with quality weighting.
///
/// Sources are registered via [`add_source`](EntropyPool::add_source) and
/// contribute to the mixed output proportionally to their quality rating.
/// A [`KeyValidator`] ensures generated keys meet strength requirements.
pub struct EntropyPool {
    sources: Vec<Box<dyn EntropySource>>,
    validator: Box<dyn KeyValidator>,
}

impl EntropyPool {
    /// Creates a new pool with the given key validator and no entropy sources.
    ///
    /// At least one source must be added via [`add_source`](Self::add_source)
    /// before calling [`harvest_mixed`](Self::harvest_mixed).
    pub fn new(validator: Box<dyn KeyValidator>) -> Self {
        Self {
            sources: Vec::new(),
            validator,
        }
    }

    /// Registers an entropy source with the pool.
    ///
    /// Sources with `quality() < 0.1` are accepted but excluded from mixing.
    pub fn add_source(&mut self, source: Box<dyn EntropySource>) {
        self.sources.push(source);
    }

    /// Returns the number of registered sources (including low-quality ones).
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Mixes entropy from all qualifying sources using quality-weighted XOR folding.
    ///
    /// Each source's contribution is scaled by its quality rating before XOR.
    /// Sources with quality below [`MIN_SOURCE_QUALITY`] are skipped.
    ///
    /// # Arguments
    ///
    /// * `seed` - Domain separation seed passed to each source's `harvest()`
    /// * `output_len` - Desired output length in bytes
    ///
    /// # Returns
    ///
    /// Mixed entropy bytes of length `output_len`. If no qualifying sources
    /// exist, returns a zero-filled buffer (caller should check via validator).
    pub fn harvest_mixed(&self, seed: &[u8], output_len: usize) -> Vec<u8> {
        let mut mixed = vec![0u8; output_len];

        for source in &self.sources {
            let quality = source.quality();
            if quality < MIN_SOURCE_QUALITY {
                continue;
            }

            let entropy = source.harvest(seed);
            for (i, &byte) in entropy.iter().enumerate() {
                if i < mixed.len() {
                    // Quality-weighted mixing: scale byte contribution by source quality
                    let weighted = (byte as f64 * quality) as u8;
                    mixed[i] ^= weighted;
                }
            }
        }

        mixed
    }

    /// Generates key material that passes the pool's validator.
    ///
    /// Repeatedly harvests and validates until a strong key is found,
    /// up to [`MAX_KEYGEN_ATTEMPTS`]. Each attempt derives a unique seed
    /// by appending the attempt counter to the base seed.
    ///
    /// # Arguments
    ///
    /// * `seed` - Base seed for entropy harvesting
    /// * `key_len` - Desired key length in bytes (typically 32 for AES-256)
    ///
    /// # Returns
    ///
    /// `Some(key)` if a valid key was generated, `None` if all attempts failed
    /// (indicates a potential entropy source failure — log and alert).
    pub fn generate_validated_key(&self, seed: &[u8], key_len: usize) -> Option<Vec<u8>> {
        for attempt in 0..MAX_KEYGEN_ATTEMPTS {
            // Derive per-attempt seed for independent key candidates
            let mut attempt_seed = seed.to_vec();
            attempt_seed.extend_from_slice(&attempt.to_le_bytes());

            let candidate = self.harvest_mixed(&attempt_seed, key_len);

            if self.validator.validate(&candidate) {
                return Some(candidate);
            }
        }

        None
    }

    /// Scores a key using the pool's validator.
    ///
    /// Convenience wrapper for [`KeyValidator::score`] using the pool's
    /// configured validator instance.
    pub fn score_key(&self, key_bytes: &[u8]) -> f64 {
        self.validator.score(key_bytes)
    }
}
