use rusqlite::Connection;
use termex_lib::storage::migrations::run_migrations;
use termex_lib::storage::Database;

fn fresh_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn
}

/// Creates a Database instance backed by a temp file (required for host_key / audit module tests).
fn temp_database() -> (Database, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open_at(dir.path().join("test.db"), None).unwrap();
    (db, dir)
}

// ══════════════════════════════════════════════════════════════
// Host Key TOFU Tests (SQL-level)
// ══════════════════════════════════════════════════════════════

#[test]
fn test_known_hosts_table_exists() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        tables.contains(&"known_hosts".to_string()),
        "known_hosts table missing"
    );
}

#[test]
fn test_known_hosts_columns() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let has_col = |col: &str| -> bool {
        conn.prepare(&format!(
            "SELECT COUNT(*) FROM pragma_table_info('known_hosts') WHERE name='{col}'"
        ))
        .and_then(|mut s| s.query_row([], |r| r.get::<_, i32>(0)))
        .map(|c| c > 0)
        .unwrap_or(false)
    };
    assert!(has_col("host"));
    assert!(has_col("port"));
    assert!(has_col("key_type"));
    assert!(has_col("fingerprint"));
    assert!(has_col("first_seen"));
    assert!(has_col("last_seen"));
}

#[test]
fn test_known_hosts_insert_and_query() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('example.com', 22, 'ssh-ed25519', 'SHA256:abc123', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();

    let fp: String = conn
        .query_row(
            "SELECT fingerprint FROM known_hosts WHERE host='example.com' AND port=22 AND key_type='ssh-ed25519'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fp, "SHA256:abc123");
}

#[test]
fn test_known_hosts_different_ports_isolated() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('example.com', 22, 'ssh-ed25519', 'fp_22', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('example.com', 2222, 'ssh-ed25519', 'fp_2222', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();

    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM known_hosts WHERE host='example.com'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

// ── Host Key TOFU: verify logic (simulated via SQL) ──

#[test]
fn test_host_key_verify_trusted() {
    // Simulate: host key already stored, same fingerprint → Trusted
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('server1.com', 22, 'ssh-ed25519', 'SHA256:known_fp', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();

    // Query matches what verify_host_key does internally
    let stored: Result<String, _> = conn.query_row(
        "SELECT fingerprint FROM known_hosts WHERE host = ?1 AND port = ?2 AND key_type = ?3",
        rusqlite::params!["server1.com", 22, "ssh-ed25519"],
        |row| row.get(0),
    );

    let old_fp = stored.unwrap();
    let new_fp = "SHA256:known_fp"; // same fingerprint
    assert_eq!(old_fp, new_fp, "Trusted: fingerprints should match");
}

#[test]
fn test_host_key_verify_new_host() {
    // Simulate: no record for host → NewHost
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    let stored: Result<String, _> = conn.query_row(
        "SELECT fingerprint FROM known_hosts WHERE host = ?1 AND port = ?2 AND key_type = ?3",
        rusqlite::params!["new-server.com", 22, "ssh-ed25519"],
        |row| row.get(0),
    );

    assert!(stored.is_err(), "NewHost: should return no rows");
}

#[test]
fn test_host_key_verify_key_changed() {
    // Simulate: stored fingerprint differs from server's → KeyChanged
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('server2.com', 22, 'ssh-ed25519', 'SHA256:old_fp', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();

    let stored_fp: String = conn
        .query_row(
            "SELECT fingerprint FROM known_hosts WHERE host = ?1 AND port = ?2 AND key_type = ?3",
            rusqlite::params!["server2.com", 22, "ssh-ed25519"],
            |row| row.get(0),
        )
        .unwrap();

    let new_fp = "SHA256:new_fp"; // different fingerprint
    assert_ne!(stored_fp, new_fp, "KeyChanged: fingerprints should differ");
}

#[test]
fn test_host_key_remove_clears_all_key_types() {
    // remove_host_key deletes by host+port, all key types
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('srv.com', 22, 'ssh-ed25519', 'fp1', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('srv.com', 22, 'ssh-rsa', 'fp2', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();

    conn.execute(
        "DELETE FROM known_hosts WHERE host = ?1 AND port = ?2",
        rusqlite::params!["srv.com", 22],
    )
    .unwrap();

    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM known_hosts WHERE host='srv.com'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_host_key_trust_preserves_first_seen() {
    // trust_host_key uses COALESCE to keep original first_seen
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('srv.com', 22, 'ssh-ed25519', 'fp_old', '2020-01-01', '2024-01-01')",
        [],
    )
    .unwrap();

    // Simulate trust_host_key's UPSERT
    conn.execute(
        "INSERT OR REPLACE INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES (?1, ?2, ?3, ?4,
           COALESCE((SELECT first_seen FROM known_hosts WHERE host=?1 AND port=?2 AND key_type=?3), ?5), ?5)",
        rusqlite::params!["srv.com", 22, "ssh-ed25519", "fp_new", "2026-04-08"],
    )
    .unwrap();

    let first_seen: String = conn
        .query_row(
            "SELECT first_seen FROM known_hosts WHERE host='srv.com' AND port=22",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(first_seen, "2020-01-01", "first_seen should be preserved");
}

// ── Host Key TOFU: via Database (full integration) ──

#[test]
fn test_host_key_remove_via_database() {
    let (db, _dir) = temp_database();

    // Insert directly
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
             VALUES ('test.com', 22, 'ssh-ed25519', 'SHA256:fp', '2024-01-01', '2024-01-01')",
            [],
        )
    })
    .unwrap();

    // Remove via module function
    termex_lib::ssh::host_key::remove_host_key(&db, "test.com", 22).unwrap();

    let count: i32 = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM known_hosts WHERE host='test.com'",
                [],
                |r| r.get(0),
            )
        })
        .unwrap();
    assert_eq!(count, 0);
}

// ══════════════════════════════════════════════════════════════
// Audit Log Tests
// ══════════════════════════════════════════════════════════════

#[test]
fn test_audit_log_table_exists() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        tables.contains(&"audit_log".to_string()),
        "audit_log table missing, got: {:?}",
        tables
    );
}

#[test]
fn test_audit_log_insert_and_query() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    conn.execute(
        "INSERT INTO audit_log (timestamp, event_type, detail) VALUES ('2024-01-01T00:00:00Z', 'ssh_connect_attempt', '{\"host\":\"1.2.3.4\"}')",
        [],
    )
    .unwrap();

    let event_type: String = conn
        .query_row(
            "SELECT event_type FROM audit_log ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(event_type, "ssh_connect_attempt");
}

#[test]
fn test_audit_log_indexes() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let indexes: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(indexes.contains(&"idx_audit_log_time".to_string()));
    assert!(indexes.contains(&"idx_audit_log_type".to_string()));
}

#[test]
fn test_audit_log_via_module() {
    let (db, _dir) = temp_database();

    // Log events via the audit module
    termex_lib::audit::log(
        &db,
        termex_lib::audit::AuditEvent::SshConnectAttempt {
            server_id: "srv1".into(),
            host: "10.0.0.1".into(),
        },
    );
    termex_lib::audit::log(
        &db,
        termex_lib::audit::AuditEvent::SshConnectSuccess {
            server_id: "srv1".into(),
            session_id: "sess1".into(),
        },
    );
    termex_lib::audit::log(
        &db,
        termex_lib::audit::AuditEvent::MasterPasswordVerified,
    );

    let count: i32 = db
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))
        })
        .unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_audit_log_stores_detail_json() {
    let (db, _dir) = temp_database();

    termex_lib::audit::log(
        &db,
        termex_lib::audit::AuditEvent::SshConnectAttempt {
            server_id: "s1".into(),
            host: "192.168.1.1".into(),
        },
    );

    let detail: Option<String> = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT detail FROM audit_log WHERE event_type = 'ssh_connect_attempt' LIMIT 1",
                [],
                |r| r.get(0),
            )
        })
        .unwrap();

    let detail = detail.expect("detail should not be NULL");
    let json: serde_json::Value = serde_json::from_str(&detail).unwrap();
    assert_eq!(json["server_id"], "s1");
    assert_eq!(json["host"], "192.168.1.1");
}

#[test]
fn test_audit_log_no_detail_for_simple_events() {
    let (db, _dir) = temp_database();

    termex_lib::audit::log(&db, termex_lib::audit::AuditEvent::MasterPasswordSet);

    let detail: Option<String> = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT detail FROM audit_log WHERE event_type = 'master_password_set' LIMIT 1",
                [],
                |r| r.get(0),
            )
        })
        .unwrap();

    assert!(detail.is_none(), "Simple events should have NULL detail");
}

#[test]
fn test_audit_log_cleanup_retention() {
    let (db, _dir) = temp_database();

    // Insert an old event (200 days ago)
    let old_time = (time::OffsetDateTime::now_utc() - time::Duration::days(200)).to_string();
    let recent_time = time::OffsetDateTime::now_utc().to_string();

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO audit_log (timestamp, event_type, detail) VALUES (?1, 'old_event', NULL)",
            rusqlite::params![old_time],
        )?;
        conn.execute(
            "INSERT INTO audit_log (timestamp, event_type, detail) VALUES (?1, 'recent_event', NULL)",
            rusqlite::params![recent_time],
        )
    })
    .unwrap();

    // Cleanup with 90-day retention
    termex_lib::audit::cleanup(&db, 90);

    let remaining: Vec<String> = db
        .with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT event_type FROM audit_log")?;
            let rows = stmt
                .query_map([], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
        .unwrap();

    assert_eq!(remaining.len(), 1, "Only recent event should remain");
    assert_eq!(remaining[0], "recent_event");
}

#[test]
fn test_audit_log_all_event_types_accepted() {
    let (db, _dir) = temp_database();

    // Test that all 16 event types can be logged without error
    let events = vec![
        termex_lib::audit::AuditEvent::MasterPasswordSet,
        termex_lib::audit::AuditEvent::MasterPasswordVerified,
        termex_lib::audit::AuditEvent::MasterPasswordFailed,
        termex_lib::audit::AuditEvent::MasterPasswordChanged,
        termex_lib::audit::AuditEvent::MasterPasswordLocked,
        termex_lib::audit::AuditEvent::SshConnectAttempt {
            server_id: "s".into(),
            host: "h".into(),
        },
        termex_lib::audit::AuditEvent::SshConnectSuccess {
            server_id: "s".into(),
            session_id: "x".into(),
        },
        termex_lib::audit::AuditEvent::SshConnectFailed {
            server_id: "s".into(),
            error: "e".into(),
        },
        termex_lib::audit::AuditEvent::SshDisconnect {
            session_id: "x".into(),
        },
        termex_lib::audit::AuditEvent::SshHostKeyNewTrusted {
            host: "h".into(),
            fingerprint: "f".into(),
        },
        termex_lib::audit::AuditEvent::SshHostKeyChanged { host: "h".into() },
        termex_lib::audit::AuditEvent::ServerCreated {
            server_id: "s".into(),
        },
        termex_lib::audit::AuditEvent::ServerDeleted {
            server_id: "s".into(),
        },
        termex_lib::audit::AuditEvent::ConfigExported,
        termex_lib::audit::AuditEvent::ConfigImported,
        termex_lib::audit::AuditEvent::CredentialAccessed {
            server_id: "s".into(),
            credential_type: "password".into(),
        },
    ];

    for event in events {
        termex_lib::audit::log(&db, event);
    }

    let count: i32 = db
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))
        })
        .unwrap();
    assert_eq!(count, 16);
}

// ══════════════════════════════════════════════════════════════
// Password Policy Tests
// ══════════════════════════════════════════════════════════════

#[test]
fn test_weak_password_rejected() {
    let result = termex_lib::crypto::password_policy::validate_master_password("123");
    assert!(result.is_err());
}

#[test]
fn test_common_password_rejected() {
    let result = termex_lib::crypto::password_policy::validate_master_password("password");
    assert!(result.is_err());
}

#[test]
fn test_short_password_rejected() {
    let result = termex_lib::crypto::password_policy::validate_master_password("Abc1");
    assert!(result.is_err());
}

#[test]
fn test_strong_password_accepted() {
    let result = termex_lib::crypto::password_policy::validate_master_password("MyStr0ng!Pass");
    assert!(result.is_ok());
}

#[test]
fn test_password_strength_scoring() {
    let weak = termex_lib::crypto::password_policy::check_strength("abc");
    assert!(weak.score < 2);

    let medium = termex_lib::crypto::password_policy::check_strength("Abcdef12");
    assert!(medium.score >= 2);

    let strong = termex_lib::crypto::password_policy::check_strength("MyV3ryStr0ng!P@ss");
    assert!(strong.score >= 3);
}

#[test]
fn test_password_with_termex_penalized() {
    let s = termex_lib::crypto::password_policy::check_strength("TermexAdmin1!");
    assert!(s.feedback.iter().any(|f| f.contains("termex")));
}

#[test]
fn test_password_no_uppercase_feedback() {
    let s = termex_lib::crypto::password_policy::check_strength("abcdefgh1");
    assert!(s.feedback.iter().any(|f| f.contains("uppercase")));
}

#[test]
fn test_password_no_digit_feedback() {
    let s = termex_lib::crypto::password_policy::check_strength("Abcdefgh");
    assert!(s.feedback.iter().any(|f| f.contains("numbers")));
}

#[test]
fn test_password_no_lowercase_feedback() {
    let s = termex_lib::crypto::password_policy::check_strength("ABCDEFGH1");
    assert!(s.feedback.iter().any(|f| f.contains("lowercase")));
}

#[test]
fn test_password_all_common_blacklisted() {
    let common = [
        "password", "12345678", "123456789", "1234567890", "qwerty123",
        "abc12345", "password1", "iloveyou", "sunshine1", "princess1",
        "admin123", "welcome1", "monkey123", "master12", "dragon12",
        "letmein1", "football1", "shadow12", "trustno1",
    ];
    for pw in common {
        let s = termex_lib::crypto::password_policy::check_strength(pw);
        assert_eq!(s.score, 0, "Common password '{}' should have score 0", pw);
    }
}

#[test]
fn test_password_common_case_insensitive() {
    // "Password" should be caught as common (case-insensitive)
    let s = termex_lib::crypto::password_policy::check_strength("Password");
    assert_eq!(s.score, 0);
}

#[test]
fn test_password_length_scoring() {
    // 8 chars = +1, 12 chars = +2, 16 chars = +3
    let s8 = termex_lib::crypto::password_policy::check_strength("aAbBcCd1");
    let s12 = termex_lib::crypto::password_policy::check_strength("aAbBcCd1eEfF");
    let s16 = termex_lib::crypto::password_policy::check_strength("aAbBcCd1eEfFgGhH");
    assert!(s12.score > s8.score, "12-char should score higher than 8-char");
    assert!(s16.score >= s12.score, "16-char should score >= 12-char");
}

#[test]
fn test_password_special_char_bonus() {
    let without = termex_lib::crypto::password_policy::check_strength("Abcdef12");
    let with = termex_lib::crypto::password_policy::check_strength("Abcdef1!");
    assert!(with.score > without.score, "Special char should boost score");
}

// ══════════════════════════════════════════════════════════════
// Zeroize Tests
// ══════════════════════════════════════════════════════════════

#[test]
fn test_kdf_returns_zeroizing() {
    let (key, _salt) = termex_lib::crypto::kdf::derive_key_new("test_password").unwrap();
    assert_eq!(key.len(), 32);
    assert!(key.iter().any(|&b| b != 0));
}

#[test]
fn test_aes_roundtrip_with_zeroizing_key() {
    let (key, _salt) = termex_lib::crypto::kdf::derive_key_new("roundtrip_test").unwrap();
    let plaintext = b"sensitive data here";
    let encrypted = termex_lib::crypto::aes::encrypt(&key, plaintext).unwrap();
    let decrypted = termex_lib::crypto::aes::decrypt(&key, &encrypted).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_kdf_deterministic_with_same_salt() {
    let salt = termex_lib::crypto::kdf::generate_salt().unwrap();
    let key1 = termex_lib::crypto::kdf::derive_key("same_password", &salt).unwrap();
    let key2 = termex_lib::crypto::kdf::derive_key("same_password", &salt).unwrap();
    assert_eq!(*key1, *key2, "Same password+salt should produce same key");
}

#[test]
fn test_kdf_different_passwords_differ() {
    let salt = termex_lib::crypto::kdf::generate_salt().unwrap();
    let key1 = termex_lib::crypto::kdf::derive_key("password_a", &salt).unwrap();
    let key2 = termex_lib::crypto::kdf::derive_key("password_b", &salt).unwrap();
    assert_ne!(*key1, *key2, "Different passwords should produce different keys");
}

#[test]
fn test_aes_encrypt_unique_nonces() {
    let (key, _) = termex_lib::crypto::kdf::derive_key_new("nonce_test").unwrap();
    let ct1 = termex_lib::crypto::aes::encrypt(&key, b"data").unwrap();
    let ct2 = termex_lib::crypto::aes::encrypt(&key, b"data").unwrap();
    assert_ne!(ct1, ct2, "Each encryption should use a unique nonce");
}

// ══════════════════════════════════════════════════════════════
// Privacy / GDPR Erasure Tests (SQL-level)
// ══════════════════════════════════════════════════════════════

#[test]
fn test_privacy_erase_clears_all_tables() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    // Insert data into multiple tables
    conn.execute(
        "INSERT INTO servers (id, name, host, port, username, auth_type, created_at, updated_at)
         VALUES ('s1', 'Test', '1.2.3.4', 22, 'root', 'password', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO known_hosts (host, port, key_type, fingerprint, first_seen, last_seen)
         VALUES ('example.com', 22, 'ssh-ed25519', 'fp1', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES ('theme', 'dark', '2024-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO audit_log (timestamp, event_type) VALUES ('2024-01-01', 'test')",
        [],
    )
    .unwrap();

    // Simulate privacy_erase_all_data (same SQL logic)
    let tables = [
        "servers", "groups", "ssh_keys", "port_forwards",
        "ai_providers", "settings", "known_hosts", "proxies",
        "connection_chain", "audit_log",
    ];
    for table in tables {
        conn.execute(&format!("DELETE FROM {}", table), []).unwrap();
    }

    // Verify all empty
    for table in tables {
        let count: i32 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "Table '{}' should be empty after erase", table);
    }
}

#[test]
fn test_privacy_erase_preserves_schema() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();

    // Erase all data
    let tables = [
        "servers", "groups", "ssh_keys", "port_forwards",
        "ai_providers", "settings", "known_hosts", "proxies",
        "connection_chain", "audit_log",
    ];
    for table in tables {
        conn.execute(&format!("DELETE FROM {}", table), []).unwrap();
    }

    // Schema should still work — insert after erase
    conn.execute(
        "INSERT INTO servers (id, name, host, port, username, auth_type, created_at, updated_at)
         VALUES ('s2', 'AfterErase', '10.0.0.1', 22, 'user', 'key', '2024-01-01', '2024-01-01')",
        [],
    )
    .unwrap();

    let name: String = conn
        .query_row("SELECT name FROM servers WHERE id='s2'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "AfterErase");
}

// ══════════════════════════════════════════════════════════════
// Auth Failure Lockout Tests (logic simulation)
// ══════════════════════════════════════════════════════════════

#[test]
fn test_lockout_logic_5_failures() {
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    // Simulate the lockout logic from commands/crypto.rs
    let failed_attempts = AtomicU32::new(0);
    let lockout_until = AtomicU64::new(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Simulate 5 failures
    for _ in 0..5 {
        let attempts = failed_attempts.fetch_add(1, Ordering::Relaxed) + 1;
        if attempts >= 10 {
            lockout_until.store(now + 300, Ordering::Relaxed);
        } else if attempts >= 5 {
            lockout_until.store(now + 30, Ordering::Relaxed);
        }
    }

    assert_eq!(failed_attempts.load(Ordering::Relaxed), 5);
    let lockout = lockout_until.load(Ordering::Relaxed);
    assert!(lockout > now, "Should be locked out after 5 failures");
    assert!(lockout <= now + 30, "Lockout should be 30s after 5 failures");
}

#[test]
fn test_lockout_logic_10_failures() {
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    let failed_attempts = AtomicU32::new(0);
    let lockout_until = AtomicU64::new(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Simulate 10 failures
    for _ in 0..10 {
        let attempts = failed_attempts.fetch_add(1, Ordering::Relaxed) + 1;
        if attempts >= 10 {
            lockout_until.store(now + 300, Ordering::Relaxed);
        } else if attempts >= 5 {
            lockout_until.store(now + 30, Ordering::Relaxed);
        }
    }

    assert_eq!(failed_attempts.load(Ordering::Relaxed), 10);
    let lockout = lockout_until.load(Ordering::Relaxed);
    assert!(lockout >= now + 300, "Lockout should be 5min after 10 failures");
}

#[test]
fn test_lockout_logic_reset_on_success() {
    use std::sync::atomic::{AtomicU32, Ordering};

    let failed_attempts = AtomicU32::new(0);

    // Simulate 3 failures
    for _ in 0..3 {
        failed_attempts.fetch_add(1, Ordering::Relaxed);
    }
    assert_eq!(failed_attempts.load(Ordering::Relaxed), 3);

    // Simulate success → reset
    failed_attempts.store(0, Ordering::Relaxed);
    assert_eq!(failed_attempts.load(Ordering::Relaxed), 0);
}

#[test]
fn test_lockout_not_triggered_under_5() {
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    let failed_attempts = AtomicU32::new(0);
    let lockout_until = AtomicU64::new(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 4 failures — should NOT trigger lockout
    for _ in 0..4 {
        let attempts = failed_attempts.fetch_add(1, Ordering::Relaxed) + 1;
        if attempts >= 10 {
            lockout_until.store(now + 300, Ordering::Relaxed);
        } else if attempts >= 5 {
            lockout_until.store(now + 30, Ordering::Relaxed);
        }
    }

    let lockout = lockout_until.load(Ordering::Relaxed);
    assert_eq!(lockout, 0, "Should NOT be locked out after only 4 failures");
}

// ══════════════════════════════════════════════════════════════
// Migration Count Tests
// ══════════════════════════════════════════════════════════════

#[test]
fn test_migration_count_is_11() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let count: i32 = conn
        .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 11);
}

#[test]
fn test_max_migration_version_is_11() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("SELECT MAX(version) FROM _migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
}
