use rusqlite::Connection;

/// All migration SQL statements, ordered by version.
/// Each entry is `(version, description, sql)`.
const MIGRATIONS: &[(i32, &str, &str)] = &[
    (1, "initial schema", MIGRATION_V1),
    (2, "ai provider max_tokens and temperature", ""),
    (3, "keychain credential storage", ""),
    (4, "keychain verification tracking", ""),
    (5, "local model integration", ""),
    (6, "network proxy support", MIGRATION_V6),
    (7, "proxy TLS support", ""),
    (8, "tmux and git sync support", ""),
    (9, "proxy command support", ""),
    (10, "connection chain support", MIGRATION_V10),
    (11, "audit log", MIGRATION_V11),
    (12, "snippet manager", MIGRATION_V12),
    (13, "session recording metadata", MIGRATION_V13),
    (14, "team collaboration", ""),
];

/// Runs all pending migrations in order.
pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Ensure the migrations tracking table exists
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version     INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at  TEXT NOT NULL
        );",
    )?;

    let current_version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for &(version, description, sql) in MIGRATIONS {
        if version > current_version {
            if !sql.is_empty() {
                conn.execute_batch(sql)?;
            }
            // Rust-based migrations (safe for columns that may already exist)
            if version == 2 {
                add_column_if_missing(conn, "ai_providers", "max_tokens", "INTEGER NOT NULL DEFAULT 4096");
                add_column_if_missing(conn, "ai_providers", "temperature", "REAL NOT NULL DEFAULT 0.7");
            }
            if version == 3 {
                add_column_if_missing(conn, "servers", "password_keychain_id", "TEXT");
                add_column_if_missing(conn, "servers", "passphrase_keychain_id", "TEXT");
                add_column_if_missing(conn, "ai_providers", "api_key_keychain_id", "TEXT");
            }
            if version == 4 {
                // Migration v4: keychain verification tracking
                // No schema changes needed; settings are stored in the KV table
                // Settings will be created on-demand:
                // - keychain_verified_at: timestamp of last successful keychain verification
                // - app_version: version from last run (for upgrade detection)
            }
            if version == 5 {
                // Migration v5: local model integration
                // Add local_model_id column to ai_providers for associating local models
                add_column_if_missing(conn, "ai_providers", "local_model_id", "TEXT");
            }
            if version == 6 {
                // Migration v6: network proxy support
                add_column_if_missing(conn, "servers", "network_proxy_id", "TEXT");
            }
            if version == 7 {
                // Migration v7: proxy TLS support (columns may already exist if v6 ran with new schema)
                add_column_if_missing(conn, "proxies", "tls_enabled", "INTEGER DEFAULT 0");
                add_column_if_missing(conn, "proxies", "tls_verify", "INTEGER DEFAULT 1");
                add_column_if_missing(conn, "proxies", "ca_cert_path", "TEXT");
                add_column_if_missing(conn, "proxies", "client_cert_path", "TEXT");
                add_column_if_missing(conn, "proxies", "client_key_path", "TEXT");
            }
            if version == 8 {
                // Migration v8: tmux persistent sessions + Git Auto Sync
                add_column_if_missing(conn, "servers", "tmux_mode", "TEXT DEFAULT 'disabled'");
                add_column_if_missing(conn, "servers", "tmux_close_action", "TEXT DEFAULT 'detach'");
                add_column_if_missing(conn, "servers", "git_sync_enabled", "INTEGER DEFAULT 0");
                add_column_if_missing(conn, "servers", "git_sync_mode", "TEXT DEFAULT 'notify'");
                add_column_if_missing(conn, "servers", "git_sync_local_path", "TEXT");
                add_column_if_missing(conn, "servers", "git_sync_remote_path", "TEXT");
            }
            if version == 9 {
                // Migration v9: ProxyCommand support
                add_column_if_missing(conn, "proxies", "command", "TEXT");
            }
            if version == 10 {
                // Migration v10: connection chain support
                // Migrate existing proxy_id / network_proxy_id to connection_chain rows
                migrate_legacy_chains(conn);
            }
            if version == 13 {
                // Migration v13: session recording metadata
                add_column_if_missing(conn, "servers", "auto_record", "INTEGER DEFAULT 0");
                add_column_if_missing(conn, "servers", "max_recording_mb", "INTEGER DEFAULT 50");
            }
            if version == 14 {
                // Migration v14: team collaboration
                add_column_if_missing(conn, "servers", "shared", "INTEGER DEFAULT 0");
                add_column_if_missing(conn, "servers", "team_id", "TEXT");
                add_column_if_missing(conn, "servers", "shared_by", "TEXT");
                add_column_if_missing(conn, "servers", "shared_at", "TEXT");
            }
            conn.execute(
                "INSERT INTO _migrations (version, description, applied_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    version,
                    description,
                    time::OffsetDateTime::now_utc().to_string(),
                ],
            )?;
        }
    }

    Ok(())
}

// ============================================================
// V1: Initial schema
// ============================================================

const MIGRATION_V1: &str = "
-- 服务器分组
CREATE TABLE groups (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    color       TEXT DEFAULT '#6366f1',
    icon        TEXT DEFAULT 'folder',
    parent_id   TEXT,
    sort_order  INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES groups(id) ON DELETE SET NULL
);

-- 服务器连接
CREATE TABLE servers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    host            TEXT NOT NULL,
    port            INTEGER DEFAULT 22,
    username        TEXT NOT NULL,
    auth_type       TEXT NOT NULL,
    password_enc    BLOB,
    password_keychain_id TEXT,
    key_path        TEXT,
    passphrase_enc  BLOB,
    passphrase_keychain_id TEXT,
    group_id        TEXT,
    sort_order      INTEGER DEFAULT 0,
    proxy_id        TEXT,
    startup_cmd     TEXT,
    encoding        TEXT DEFAULT 'UTF-8',
    tags            TEXT,
    last_connected  TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE SET NULL
);

-- SSH 密钥管理
CREATE TABLE ssh_keys (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    key_type        TEXT NOT NULL,
    bits            INTEGER,
    file_path       TEXT NOT NULL,
    public_key      TEXT,
    has_passphrase  INTEGER DEFAULT 0,
    passphrase_enc  BLOB,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- 端口转发规则
CREATE TABLE port_forwards (
    id              TEXT PRIMARY KEY,
    server_id       TEXT NOT NULL,
    forward_type    TEXT NOT NULL,
    local_host      TEXT DEFAULT '127.0.0.1',
    local_port      INTEGER NOT NULL,
    remote_host     TEXT,
    remote_port     INTEGER,
    auto_start      INTEGER DEFAULT 0,
    enabled         INTEGER DEFAULT 1,
    created_at      TEXT NOT NULL,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

-- AI Provider 配置
CREATE TABLE ai_providers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    provider_type   TEXT NOT NULL,
    api_key_enc     BLOB,
    api_key_keychain_id TEXT,
    api_base_url    TEXT,
    model           TEXT NOT NULL,
    max_tokens      INTEGER NOT NULL DEFAULT 4096,
    temperature     REAL NOT NULL DEFAULT 0.7,
    is_default      INTEGER DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- 应用设置 (KV 存储)
CREATE TABLE settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- 主机指纹
CREATE TABLE known_hosts (
    host            TEXT NOT NULL,
    port            INTEGER NOT NULL,
    key_type        TEXT NOT NULL,
    fingerprint     TEXT NOT NULL,
    first_seen      TEXT NOT NULL,
    last_seen       TEXT NOT NULL,
    PRIMARY KEY (host, port, key_type)
);

-- 索引
CREATE INDEX idx_servers_group ON servers(group_id);
CREATE INDEX idx_servers_name ON servers(name);
CREATE INDEX idx_port_forwards_server ON port_forwards(server_id);
CREATE INDEX idx_ai_providers_default ON ai_providers(is_default);
";


// ============================================================
// V6: Network proxy support
// ============================================================

const MIGRATION_V6: &str = "
CREATE TABLE IF NOT EXISTS proxies (
    id                      TEXT PRIMARY KEY,
    name                    TEXT NOT NULL,
    proxy_type              TEXT NOT NULL,
    host                    TEXT NOT NULL,
    port                    INTEGER NOT NULL,
    username                TEXT,
    password_enc            BLOB,
    password_keychain_id    TEXT,
    tls_enabled             INTEGER DEFAULT 0,
    tls_verify              INTEGER DEFAULT 1,
    ca_cert_path            TEXT,
    client_cert_path        TEXT,
    client_key_path         TEXT,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);
";

// ============================================================
// V10: Connection chain support
// ============================================================

const MIGRATION_V10: &str = "
CREATE TABLE IF NOT EXISTS connection_chain (
    id          TEXT PRIMARY KEY,
    server_id   TEXT NOT NULL,
    position    INTEGER NOT NULL,
    hop_type    TEXT NOT NULL,
    hop_id      TEXT NOT NULL,
    phase       TEXT NOT NULL DEFAULT 'pre',
    created_at  TEXT NOT NULL,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_chain_server ON connection_chain(server_id, position);
";

// ============================================================
// V11: Audit log
// ============================================================

const MIGRATION_V11: &str = "
CREATE TABLE IF NOT EXISTS audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    detail      TEXT,
    source_ip   TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_log_time ON audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_log_type ON audit_log(event_type);
";

// ============================================================
// V12: Snippet manager
// ============================================================

const MIGRATION_V12: &str = "
CREATE TABLE IF NOT EXISTS snippet_folders (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    parent_id  TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_snippet_folders_parent ON snippet_folders(parent_id);

CREATE TABLE IF NOT EXISTS snippets (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    description  TEXT,
    command      TEXT NOT NULL,
    tags         TEXT,
    folder_id    TEXT,
    is_favorite  INTEGER NOT NULL DEFAULT 0,
    usage_count  INTEGER NOT NULL DEFAULT 0,
    last_used_at TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_snippets_folder ON snippets(folder_id);
CREATE INDEX IF NOT EXISTS idx_snippets_favorite ON snippets(is_favorite);
";

/// Migrates existing `proxy_id` / `network_proxy_id` fields to `connection_chain` rows.
/// Called once during migration V10. Servers with neither field are skipped.
fn migrate_legacy_chains(conn: &Connection) {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, proxy_id, network_proxy_id FROM servers
         WHERE (proxy_id IS NOT NULL AND proxy_id != '')
            OR (network_proxy_id IS NOT NULL AND network_proxy_id != '')",
    ) else {
        return;
    };

    let Ok(rows_iter) = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    }) else {
        return;
    };
    let rows: Vec<(String, Option<String>, Option<String>)> =
        rows_iter.filter_map(|r| r.ok()).collect();

    let now = time::OffsetDateTime::now_utc().to_string();

    for (server_id, proxy_id, network_proxy_id) in rows {
        let mut position = 0i32;

        // Network proxy comes first in the chain (position 0)
        if let Some(ref np_id) = network_proxy_id {
            if !np_id.is_empty() {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO connection_chain (id, server_id, position, hop_type, hop_id, phase, created_at)
                     VALUES (?1, ?2, ?3, 'proxy', ?4, 'pre', ?5)",
                    rusqlite::params![id, server_id, position, np_id, now],
                );
                position += 1;
            }
        }

        // SSH bastion comes after network proxy (position 1)
        if let Some(ref p_id) = proxy_id {
            if !p_id.is_empty() {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO connection_chain (id, server_id, position, hop_type, hop_id, phase, created_at)
                     VALUES (?1, ?2, ?3, 'ssh', ?4, 'pre', ?5)",
                    rusqlite::params![id, server_id, position, p_id, now],
                );
            }
        }
    }
}

/// Adds a column to a table if it doesn't already exist (idempotent).
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, col_type: &str) {
    let has_col: bool = conn
        .prepare(&format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name='{column}'"))
        .and_then(|mut s| s.query_row([], |r| r.get::<_, i32>(0)))
        .map(|c| c > 0)
        .unwrap_or(false);

    if !has_col {
        let _ = conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {col_type};"));
    }
}

const MIGRATION_V13: &str = "
CREATE TABLE IF NOT EXISTS recordings (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    server_id       TEXT NOT NULL,
    server_name     TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    file_size       INTEGER DEFAULT 0,
    duration_ms     INTEGER DEFAULT 0,
    cols            INTEGER NOT NULL,
    rows            INTEGER NOT NULL,
    event_count     INTEGER DEFAULT 0,
    summary         TEXT,
    auto_recorded   INTEGER DEFAULT 0,
    started_at      TEXT NOT NULL,
    ended_at        TEXT,
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_recordings_server ON recordings(server_id, started_at);
CREATE INDEX IF NOT EXISTS idx_recordings_date ON recordings(started_at);
";
