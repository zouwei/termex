use termex_lib::team::crypto::{
    create_verify_token, derive_team_key, generate_team_salt, re_encrypt,
    team_decrypt, team_encrypt, verify_passphrase, VERIFY_PLAINTEXT,
};

#[test]
fn test_derive_team_key_deterministic() {
    let salt = [1u8; 16];
    let key1 = derive_team_key("my-team-passphrase", &salt).unwrap();
    let key2 = derive_team_key("my-team-passphrase", &salt).unwrap();
    assert_eq!(*key1, *key2, "same passphrase + salt must produce same key");
}

#[test]
fn test_derive_team_key_different_passphrase() {
    let salt = [2u8; 16];
    let key1 = derive_team_key("passphrase-a", &salt).unwrap();
    let key2 = derive_team_key("passphrase-b", &salt).unwrap();
    assert_ne!(*key1, *key2);
}

#[test]
fn test_derive_team_key_different_salt() {
    let salt1 = [3u8; 16];
    let salt2 = [4u8; 16];
    let key1 = derive_team_key("same-passphrase", &salt1).unwrap();
    let key2 = derive_team_key("same-passphrase", &salt2).unwrap();
    assert_ne!(*key1, *key2);
}

#[test]
fn test_generate_salt_unique() {
    let s1 = generate_team_salt().unwrap();
    let s2 = generate_team_salt().unwrap();
    assert_ne!(s1, s2, "salts should be random and unique");
}

#[test]
fn test_encrypt_decrypt_roundtrip() {
    let salt = [5u8; 16];
    let key = derive_team_key("roundtrip-test", &salt).unwrap();
    let plaintext = "Hello, team! 你好团队";
    let encrypted = team_encrypt(&key, plaintext).unwrap();
    let decrypted = team_decrypt(&key, &encrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_encrypt_decrypt_empty_string() {
    let salt = [6u8; 16];
    let key = derive_team_key("empty-test", &salt).unwrap();
    let encrypted = team_encrypt(&key, "").unwrap();
    let decrypted = team_decrypt(&key, &encrypted).unwrap();
    assert_eq!(decrypted, "");
}

#[test]
fn test_decrypt_wrong_key_fails() {
    let salt = [7u8; 16];
    let key1 = derive_team_key("correct-key", &salt).unwrap();
    let key2 = derive_team_key("wrong-key", &salt).unwrap();
    let encrypted = team_encrypt(&key1, "secret data").unwrap();
    assert!(team_decrypt(&key2, &encrypted).is_err());
}

#[test]
fn test_decrypt_invalid_base64_fails() {
    let salt = [8u8; 16];
    let key = derive_team_key("test", &salt).unwrap();
    assert!(team_decrypt(&key, "not-valid-base64!!!").is_err());
}

#[test]
fn test_decrypt_short_data_fails() {
    let salt = [9u8; 16];
    let key = derive_team_key("test", &salt).unwrap();
    // Valid base64 but too short for nonce(12) + tag(16)
    assert!(team_decrypt(&key, "AQIDBA==").is_err());
}

#[test]
fn test_verify_passphrase_correct() {
    let salt = [10u8; 16];
    let key = derive_team_key("verify-test", &salt).unwrap();
    let token = create_verify_token(&key).unwrap();
    assert!(verify_passphrase(&key, &token));
}

#[test]
fn test_verify_passphrase_wrong() {
    let salt = [11u8; 16];
    let correct_key = derive_team_key("correct", &salt).unwrap();
    let wrong_key = derive_team_key("wrong", &salt).unwrap();
    let token = create_verify_token(&correct_key).unwrap();
    assert!(!verify_passphrase(&wrong_key, &token));
}

#[test]
fn test_re_encrypt() {
    let salt = [12u8; 16];
    let old_key = derive_team_key("old-passphrase", &salt).unwrap();
    let new_salt = [13u8; 16];
    let new_key = derive_team_key("new-passphrase", &new_salt).unwrap();

    let original = "sensitive-password-123";
    let encrypted_old = team_encrypt(&old_key, original).unwrap();

    // Re-encrypt with new key
    let encrypted_new = re_encrypt(&old_key, &new_key, &encrypted_old).unwrap();

    // Old key cannot decrypt new ciphertext
    assert!(team_decrypt(&old_key, &encrypted_new).is_err());

    // New key can decrypt
    let decrypted = team_decrypt(&new_key, &encrypted_new).unwrap();
    assert_eq!(decrypted, original);
}
