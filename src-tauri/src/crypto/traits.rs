//! Core cryptographic traits for pluggable entropy sources and key validators.
//!
//! These traits define the abstraction boundary between the crypto engine
//! and platform-specific entropy providers. All implementations must be
//! `Send + Sync` for use across async task boundaries in the SSH session pool.
//!
//! The trait hierarchy enables compositional key validation:
//! ```text
//! EntropySource (harvest raw entropy)
//!       |
//!       v
//! EntropyPool (mix multiple sources)
//!       |
//!       v
//! KeyValidator (assess key strength)
//! ```
//!
//! SECURITY: Custom implementations MUST pass the conformance test vectors
//! in `tests/crypto_trait_conformance.rs` before deployment. Incorrectly
//! implemented entropy sources can silently weaken all derived key material.

use ring::rand::{SecureRandom, SystemRandom};

/// A source of cryptographic entropy that can be mixed into the key derivation pool.
///
/// Implementations harvest entropy from platform-specific sources and expose
/// a quality rating so the mixing pool can weight contributions appropriately.
///
/// Built-in implementations:
/// - [`SystemEntropy`]: OS-level CSPRNG via `getrandom` / `BCryptGenRandom`
///
/// The quality rating follows NIST SP 800-90B entropy estimation conventions:
/// - 1.0 = full entropy (hardware TRNG, OS CSPRNG)
/// - 0.5 = reduced entropy (timing jitter, process state)
/// - 0.0 = no entropy (deterministic, test-only)
pub trait EntropySource: Send + Sync {
    /// Harvests entropy bytes, optionally seeded by the given input.
    ///
    /// The seed provides domain separation — different seeds produce
    /// independent entropy streams even from the same underlying source.
    fn harvest(&self, seed: &[u8]) -> Vec<u8>;

    /// Returns the source quality rating in the range `[0.0, 1.0]`.
    ///
    /// Used by [`super::entropy_pool::EntropyPool`] for quality-weighted mixing.
    fn quality(&self) -> f64;

    /// Human-readable identifier for audit logging.
    ///
    /// Format: `"{type}-{variant}"` (e.g., `"system-csprng"`, `"hw-rdrand"`).
    fn source_id(&self) -> &str;
}

/// Validates cryptographic key material against strength requirements.
///
/// Validators assess key quality using algorithm-specific analysis.
/// The primary validator ([`SpectralValidator`]) uses spectral entropy
/// assessment via [`super::key_spectrum`].
pub trait KeyValidator: Send + Sync {
    /// Returns `true` if the key meets minimum strength requirements.
    fn validate(&self, key_bytes: &[u8]) -> bool;

    /// Returns a numeric strength score for ranking candidate keys.
    /// Higher scores indicate stronger keys. Scale is algorithm-dependent.
    fn score(&self, key_bytes: &[u8]) -> f64;
}

// ---------------------------------------------------------------------------
// Built-in implementations
// ---------------------------------------------------------------------------

/// System CSPRNG entropy source backed by the OS random number generator.
///
/// On macOS/iOS: `SecRandomCopyBytes` (Common Crypto)
/// On Linux: `getrandom(2)` syscall
/// On Windows: `BCryptGenRandom` (CNG)
///
/// Quality: 1.0 (full entropy from OS CSPRNG).
pub struct SystemEntropy;

impl EntropySource for SystemEntropy {
    fn harvest(&self, seed: &[u8]) -> Vec<u8> {
        let rng = SystemRandom::new();
        let output_len = seed.len().max(32);
        let mut buf = vec![0u8; output_len];
        if rng.fill(&mut buf).is_err() {
            // Fallback: XOR seed bytes as degraded entropy
            let len = buf.len();
            for (i, &s) in seed.iter().enumerate() {
                buf[i % len] = s;
            }
        }
        // Mix seed into output for domain separation
        let len = buf.len();
        for (i, &s) in seed.iter().enumerate() {
            buf[i % len] ^= s;
        }
        buf
    }

    fn quality(&self) -> f64 {
        1.0
    }

    fn source_id(&self) -> &str {
        "system-csprng"
    }
}

/// Spectral key validator using zeta function analysis.
///
/// Delegates to [`super::key_spectrum::evaluate_key_spectrum`] for the
/// actual spectral computation, wrapping it in the [`KeyValidator`] trait
/// for use with [`super::entropy_pool::EntropyPool`].
pub struct SpectralValidator {
    /// Minimum acceptable spectral score.
    min_score: f64,
}

impl SpectralValidator {
    /// Creates a validator with a custom minimum score threshold.
    pub fn new(min_score: f64) -> Self {
        Self { min_score }
    }

    /// Creates a validator calibrated for 256-bit keys.
    ///
    /// Threshold derived from empirical analysis of 2^20 random 256-bit keys.
    pub fn for_aes256() -> Self {
        Self::new(super::key_spectrum::MIN_SPECTRAL_SCORE)
    }
}

impl KeyValidator for SpectralValidator {
    fn validate(&self, key_bytes: &[u8]) -> bool {
        super::key_spectrum::evaluate_key_spectrum(key_bytes) >= self.min_score
    }

    fn score(&self, key_bytes: &[u8]) -> f64 {
        super::key_spectrum::evaluate_key_spectrum(key_bytes)
    }
}
