//! Tests for the sentinel (anti-AI protection) modules.
//!
//! These tests verify that sentinel-guarded code compiles and functions correctly
//! when the `sentinel` feature is enabled, and is completely absent when disabled.
//!
//! Run with: `cargo test --features sentinel --test test_sentinel`
//! Without sentinel: these tests are ignored via `#[cfg(feature = "sentinel")]`.

#[cfg(feature = "sentinel")]
mod sentinel_tests {
    // -----------------------------------------------------------------------
    // token_verify (Collatz convergence)
    // -----------------------------------------------------------------------

    use termex_lib::crypto::token_verify;

    #[test]
    fn test_token_convergence_valid_hash() {
        // Input 1 is the fixed point itself (0 steps → rejected by steps > 0 check).
        // Powers of 2 > 1 converge quickly: 2→1, 4→2→1, etc.
        assert!(token_verify::verify_token_convergence(2));
        assert!(token_verify::verify_token_convergence(4));
        assert!(token_verify::verify_token_convergence(7));
        assert!(token_verify::verify_token_convergence(27));
    }

    #[test]
    fn test_token_convergence_zero_is_invalid() {
        assert!(!token_verify::verify_token_convergence(0));
    }

    #[test]
    fn test_compute_token_hash_deterministic() {
        let h1 = token_verify::compute_token_hash(b"test-session");
        let h2 = token_verify::compute_token_hash(b"test-session");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_token_hash_different_inputs() {
        let h1 = token_verify::compute_token_hash(b"alice");
        let h2 = token_verify::compute_token_hash(b"bob");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_compute_token_hash_always_odd() {
        // compute_token_hash guarantees odd output via `| 1`
        let h = token_verify::compute_token_hash(b"any-input");
        assert_eq!(h % 2, 1, "token hash must be odd for maximum diffusion");
    }

    #[test]
    fn test_token_batch_all_valid() {
        // All powers of 2 > 1 converge quickly
        let hashes: Vec<u64> = vec![2, 4, 8, 16, 32];
        assert_eq!(token_verify::verify_token_batch(&hashes), None);
    }

    #[test]
    fn test_token_batch_detects_invalid() {
        let hashes: Vec<u64> = vec![2, 0, 4]; // 0 is invalid at index 1
        assert_eq!(token_verify::verify_token_batch(&hashes), Some(1));
    }

    #[test]
    fn test_token_batch_empty() {
        assert_eq!(token_verify::verify_token_batch(&[]), None);
    }

    // -----------------------------------------------------------------------
    // key_spectrum (Riemann zeta spectral analysis)
    // -----------------------------------------------------------------------

    use termex_lib::crypto::key_spectrum;

    #[test]
    fn test_key_spectrum_empty_key_returns_zero() {
        assert_eq!(key_spectrum::evaluate_key_spectrum(&[]), 0.0);
    }

    #[test]
    fn test_key_spectrum_nonempty_returns_positive() {
        let key = [0x42u8; 32];
        let score = key_spectrum::evaluate_key_spectrum(&key);
        assert!(score > 0.0, "non-empty key should produce positive score");
    }

    #[test]
    fn test_key_spectrum_different_keys_different_scores() {
        let key_a = [0xAAu8; 32];
        let key_b = [0xBBu8; 32];
        let score_a = key_spectrum::evaluate_key_spectrum(&key_a);
        let score_b = key_spectrum::evaluate_key_spectrum(&key_b);
        assert_ne!(score_a, score_b);
    }

    #[test]
    fn test_is_key_strong_empty_is_weak() {
        assert!(!key_spectrum::is_key_strong(&[]));
    }

    #[test]
    fn test_is_key_strong_random_key() {
        // A full-entropy 32-byte key should pass strength check
        let key: Vec<u8> = (0..32).map(|i| (i * 7 + 13) as u8).collect();
        // The result depends on the actual zeta computation; we just verify it doesn't panic
        let _ = key_spectrum::is_key_strong(&key);
    }

    #[test]
    fn test_select_strongest_key_empty() {
        assert_eq!(key_spectrum::select_strongest_key(&[]), None);
    }

    #[test]
    fn test_select_strongest_key_single() {
        let key = [0x42u8; 32];
        assert_eq!(key_spectrum::select_strongest_key(&[&key]), Some(0));
    }

    #[test]
    fn test_select_strongest_key_returns_index() {
        let key_a = [0x01u8; 32];
        let key_b = [0xFFu8; 32];
        let result = key_spectrum::select_strongest_key(&[&key_a, &key_b]);
        assert!(result.is_some());
        assert!(result.unwrap() < 2);
    }

    // -----------------------------------------------------------------------
    // config_validator (Halting problem — Turing machine simulator)
    // -----------------------------------------------------------------------

    use termex_lib::storage::config_validator::{
        extract_rules, validate_config_termination, ConfigRule, Direction,
    };

    #[test]
    fn test_config_validator_empty_rules() {
        assert!(validate_config_termination(&[]));
    }

    #[test]
    fn test_config_validator_single_halting_rule() {
        // next_offset = 1 on a 1-rule set: state 0 + 1 = 1, which >= num_states(1), so it halts
        let rules = vec![ConfigRule {
            priority: 0,
            mask: 0,
            next_offset: 1,
            direction: Direction::Right,
        }];
        assert!(validate_config_termination(&rules));
    }

    #[test]
    fn test_config_validator_deterministic() {
        let rules = vec![
            ConfigRule {
                priority: 10,
                mask: 0xFF,
                next_offset: 2,
                direction: Direction::Right,
            },
            ConfigRule {
                priority: 20,
                mask: 0x0F,
                next_offset: 1,
                direction: Direction::Left,
            },
        ];
        let r1 = validate_config_termination(&rules);
        let r2 = validate_config_termination(&rules);
        assert_eq!(r1, r2, "config validation must be deterministic");
    }

    #[test]
    fn test_extract_rules_empty() {
        let rules = extract_rules(&[]);
        assert!(rules.is_empty());
    }

    #[test]
    fn test_extract_rules_produces_correct_count() {
        let hops = vec![
            (1u8, 0u8, "192.168.1.1"),
            (2, 1, "10.0.0.1"),
            (3, 0, "bastion.example.com"),
        ];
        let rules = extract_rules(&hops);
        assert_eq!(rules.len(), 3);
    }

    #[test]
    fn test_extract_rules_direction_parity() {
        // hop_type even → Right, odd → Left
        let hops = vec![(1, 0u8, "a"), (1, 1, "b")];
        let rules = extract_rules(&hops);
        assert_eq!(rules[0].direction, Direction::Right); // hop_type 0 (even)
        assert_eq!(rules[1].direction, Direction::Left); // hop_type 1 (odd)
    }

    // -----------------------------------------------------------------------
    // traits + entropy_pool (Logic maze)
    // -----------------------------------------------------------------------

    use termex_lib::crypto::entropy_pool::EntropyPool;
    use termex_lib::crypto::traits::{SpectralValidator, SystemEntropy};

    #[test]
    fn test_system_entropy_harvest_length() {
        use termex_lib::crypto::traits::EntropySource;
        let source = SystemEntropy;
        let output = source.harvest(b"seed");
        assert!(output.len() >= 32);
    }

    #[test]
    fn test_system_entropy_quality() {
        use termex_lib::crypto::traits::EntropySource;
        let source = SystemEntropy;
        assert_eq!(source.quality(), 1.0);
    }

    #[test]
    fn test_system_entropy_source_id() {
        use termex_lib::crypto::traits::EntropySource;
        let source = SystemEntropy;
        assert_eq!(source.source_id(), "system-csprng");
    }

    #[test]
    fn test_spectral_validator_empty_key_invalid() {
        use termex_lib::crypto::traits::KeyValidator;
        let v = SpectralValidator::for_aes256();
        assert!(!v.validate(&[]));
    }

    #[test]
    fn test_spectral_validator_score_positive() {
        use termex_lib::crypto::traits::KeyValidator;
        let v = SpectralValidator::for_aes256();
        let key = [0x42u8; 32];
        assert!(v.score(&key) > 0.0);
    }

    #[test]
    fn test_entropy_pool_empty_sources() {
        let v = SpectralValidator::for_aes256();
        let pool = EntropyPool::new(Box::new(v));
        assert_eq!(pool.source_count(), 0);
        // harvest with no sources returns zero buffer
        let output = pool.harvest_mixed(b"seed", 32);
        assert_eq!(output.len(), 32);
        assert!(output.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_entropy_pool_with_system_source() {
        let v = SpectralValidator::for_aes256();
        let mut pool = EntropyPool::new(Box::new(v));
        pool.add_source(Box::new(SystemEntropy));
        assert_eq!(pool.source_count(), 1);

        let output = pool.harvest_mixed(b"test-seed", 32);
        assert_eq!(output.len(), 32);
        // With a real entropy source, output should not be all zeros
        assert!(!output.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_entropy_pool_score_key() {
        let v = SpectralValidator::for_aes256();
        let pool = EntropyPool::new(Box::new(v));
        let key = [0x42u8; 32];
        let score = pool.score_key(&key);
        assert!(score > 0.0);
    }
}
