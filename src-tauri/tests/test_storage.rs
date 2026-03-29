use rusqlite::Connection;
use termex_lib::storage::migrations::run_migrations;

fn fresh_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn
}

// ── Migration Tests ──

#[test]
fn test_migrations_idempotent() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    run_migrations(&conn).unwrap();
    let version: i32 = conn
        .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 4);
}

#[test]
fn test_all_migrations_applied() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let count: i32 = conn
        .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 4);
}

#[test]
fn test_v1_creates_all_tables() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let tables: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    for expected in &["servers", "port_forwards", "ai_providers", "settings", "known_hosts"] {
        assert!(tables.contains(&expected.to_string()), "missing table: {expected}, got: {:?}", tables);
    }
}

#[test]
fn test_servers_has_keychain_columns() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let has_col = |col: &str| -> bool {
        conn.prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('servers') WHERE name='{col}'"))
            .and_then(|mut s| s.query_row([], |r| r.get::<_, i32>(0)))
            .map(|c| c > 0)
            .unwrap_or(false)
    };
    assert!(has_col("password_keychain_id"));
    assert!(has_col("passphrase_keychain_id"));
    assert!(has_col("password_enc"));
}

#[test]
fn test_ai_providers_has_keychain_and_token_columns() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    let has_col = |col: &str| -> bool {
        conn.prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('ai_providers') WHERE name='{col}'"))
            .and_then(|mut s| s.query_row([], |r| r.get::<_, i32>(0)))
            .map(|c| c > 0)
            .unwrap_or(false)
    };
    assert!(has_col("api_key_keychain_id"));
    assert!(has_col("max_tokens"));
    assert!(has_col("temperature"));
    assert!(has_col("api_key_enc"));
}

#[test]
fn test_v2_upgrade_defaults() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO ai_providers (id, name, provider_type, model, created_at, updated_at)
         VALUES ('test', 'Test', 'openai', 'gpt-4o', '2024-01-01', '2024-01-01')",
        [],
    ).unwrap();
    let (max_tokens, temperature): (i32, f64) = conn
        .query_row("SELECT max_tokens, temperature FROM ai_providers WHERE id='test'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(max_tokens, 4096);
    assert!((temperature - 0.7).abs() < 0.001);
}

#[test]
fn test_server_insert_with_keychain_fields() {
    let conn = fresh_db();
    run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO servers (id, name, host, port, username, auth_type,
            password_keychain_id, passphrase_keychain_id, created_at, updated_at)
         VALUES ('s1', 'Test', '1.2.3.4', 22, 'root', 'password',
            'termex:ssh:password:s1', NULL, '2024-01-01', '2024-01-01')",
        [],
    ).unwrap();
    let kc_id: Option<String> = conn
        .query_row("SELECT password_keychain_id FROM servers WHERE id='s1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(kc_id, Some("termex:ssh:password:s1".to_string()));
    let pp_id: Option<String> = conn
        .query_row("SELECT passphrase_keychain_id FROM servers WHERE id='s1'", [], |r| r.get(0))
        .unwrap();
    assert!(pp_id.is_none());
}

// ── DB Open Test ──

#[test]
fn test_open_in_memory() {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    run_migrations(&conn).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(tables.contains(&"groups".to_string()));
    assert!(tables.contains(&"servers".to_string()));
    assert!(tables.contains(&"settings".to_string()));
    assert!(tables.contains(&"known_hosts".to_string()));
}
