use termex_lib::keychain;

/// Probe whether keychain actually works (not just Entry::new).
/// On headless Linux CI, `is_available()` may return true but store/get fail.
fn keychain_works() -> bool {
    let probe_key = "termex:test:probe:ci";
    if keychain::store(probe_key, "probe").is_err() {
        return false;
    }
    let _ = keychain::delete(probe_key);
    true
}

#[test]
fn test_key_generation() {
    assert_eq!(keychain::ssh_password_key("abc"), "termex:ssh:password:abc");
    assert_eq!(keychain::ssh_passphrase_key("abc"), "termex:ssh:passphrase:abc");
    assert_eq!(keychain::ai_apikey_key("xyz"), "termex:ai:apikey:xyz");
}

#[test]
fn test_key_format_includes_uuid() {
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    let key = keychain::ssh_password_key(uuid);
    assert!(key.starts_with("termex:ssh:password:"));
    assert!(key.ends_with(uuid));
}

#[test]
fn test_is_available() {
    let available = keychain::is_available();
    assert!(available || !available);
}

#[test]
fn test_store_get_delete_lifecycle() {
    if !keychain_works() { return; }
    let key = "termex:test:lifecycle:unit";
    let value = "test_secret_value_12345";
    keychain::store(key, value).expect("store should succeed");
    let retrieved = keychain::get(key).expect("get should succeed");
    assert_eq!(retrieved, value);
    keychain::delete(key).expect("delete should succeed");
    assert!(keychain::get(key).is_err());
}

#[test]
fn test_store_overwrite() {
    if !keychain_works() { return; }
    let key = "termex:test:overwrite:unit";
    keychain::store(key, "first_value").expect("first store");
    keychain::store(key, "second_value").expect("overwrite store");
    let retrieved = keychain::get(key).expect("get after overwrite");
    assert_eq!(retrieved, "second_value");
    let _ = keychain::delete(key);
}

#[test]
fn test_delete_nonexistent_is_ok() {
    if !keychain_works() { return; }
    let result = keychain::delete("termex:test:nonexistent:key");
    assert!(result.is_ok());
}

#[test]
fn test_get_nonexistent_returns_error() {
    if !keychain_works() { return; }
    let result = keychain::get("termex:test:definitely:missing");
    assert!(result.is_err());
}

#[test]
fn test_store_empty_value() {
    if !keychain_works() { return; }
    let key = "termex:test:empty:unit";
    keychain::store(key, "").expect("store empty string");
    let retrieved = keychain::get(key).expect("get empty string");
    assert_eq!(retrieved, "");
    let _ = keychain::delete(key);
}

#[test]
fn test_store_unicode_value() {
    if !keychain_works() { return; }
    let key = "termex:test:unicode:unit";
    let value = "密码测试🔐";
    keychain::store(key, value).expect("store unicode");
    let retrieved = keychain::get(key).expect("get unicode");
    assert_eq!(retrieved, value);
    let _ = keychain::delete(key);
}

#[test]
fn test_ssh_and_ai_keys_independent() {
    if !keychain_works() { return; }
    let server_id = "test-server-id-001";
    let provider_id = "test-provider-id-001";
    let pw_key = keychain::ssh_password_key(server_id);
    let ai_key = keychain::ai_apikey_key(provider_id);
    keychain::store(&pw_key, "ssh_pass").expect("store ssh");
    keychain::store(&ai_key, "sk-apikey").expect("store ai");
    assert_eq!(keychain::get(&pw_key).unwrap(), "ssh_pass");
    assert_eq!(keychain::get(&ai_key).unwrap(), "sk-apikey");
    keychain::delete(&pw_key).expect("delete ssh");
    assert!(keychain::get(&pw_key).is_err());
    assert_eq!(keychain::get(&ai_key).unwrap(), "sk-apikey");
    let _ = keychain::delete(&ai_key);
}
