//! Database CRUD operations for network proxy configurations.

use crate::storage::Database;
use crate::storage::models::Proxy;

const SELECT_COLS: &str =
    "id, name, proxy_type, host, port, username,
     password_enc, password_keychain_id,
     tls_enabled, tls_verify, ca_cert_path, client_cert_path, client_key_path,
     command, created_at, updated_at";

fn row_to_proxy(row: &rusqlite::Row) -> rusqlite::Result<Proxy> {
    Ok(Proxy {
        id: row.get(0)?,
        name: row.get(1)?,
        proxy_type: row.get(2)?,
        host: row.get(3)?,
        port: row.get(4)?,
        username: row.get(5)?,
        password_enc: row.get(6)?,
        password_keychain_id: row.get(7)?,
        tls_enabled: row.get(8)?,
        tls_verify: row.get(9)?,
        ca_cert_path: row.get(10)?,
        client_cert_path: row.get(11)?,
        client_key_path: row.get(12)?,
        command: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

/// Lists all proxy configurations.
pub fn list(db: &Database) -> Result<Vec<Proxy>, String> {
    db.with_conn(|conn| {
        let sql = format!("SELECT {} FROM proxies ORDER BY name", SELECT_COLS);
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| row_to_proxy(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    })
    .map_err(|e| e.to_string())
}

/// Gets a single proxy by ID.
pub fn get(db: &Database, id: &str) -> Result<Proxy, String> {
    db.with_conn(|conn| {
        let sql = format!("SELECT {} FROM proxies WHERE id = ?1", SELECT_COLS);
        conn.query_row(&sql, rusqlite::params![id], |row| row_to_proxy(row))
    })
    .map_err(|e| e.to_string())
}

/// Creates a new proxy configuration.
pub fn create(
    db: &Database,
    id: &str,
    name: &str,
    proxy_type: &str,
    host: &str,
    port: i32,
    username: Option<&str>,
    password_enc: Option<&[u8]>,
    password_keychain_id: Option<&str>,
    tls_enabled: bool,
    tls_verify: bool,
    ca_cert_path: Option<&str>,
    client_cert_path: Option<&str>,
    client_key_path: Option<&str>,
    command: Option<&str>,
    now: &str,
) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO proxies (id, name, proxy_type, host, port, username,
                password_enc, password_keychain_id,
                tls_enabled, tls_verify, ca_cert_path, client_cert_path, client_key_path,
                command, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
            rusqlite::params![
                id, name, proxy_type, host, port, username,
                password_enc, password_keychain_id,
                tls_enabled, tls_verify, ca_cert_path, client_cert_path, client_key_path,
                command, now,
            ],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

/// Updates an existing proxy configuration.
pub fn update(
    db: &Database,
    id: &str,
    name: &str,
    proxy_type: &str,
    host: &str,
    port: i32,
    username: Option<&str>,
    password_enc: Option<&[u8]>,
    password_keychain_id: Option<&str>,
    tls_enabled: bool,
    tls_verify: bool,
    ca_cert_path: Option<&str>,
    client_cert_path: Option<&str>,
    client_key_path: Option<&str>,
    command: Option<&str>,
    now: &str,
) -> Result<(), String> {
    db.with_conn(|conn| {
        let affected = conn.execute(
            "UPDATE proxies SET name=?1, proxy_type=?2, host=?3, port=?4, username=?5,
                password_enc=COALESCE(?6, password_enc),
                password_keychain_id=COALESCE(?7, password_keychain_id),
                tls_enabled=?8, tls_verify=?9,
                ca_cert_path=?10, client_cert_path=?11, client_key_path=?12,
                command=?13, updated_at=?14
             WHERE id=?15",
            rusqlite::params![
                name, proxy_type, host, port, username,
                password_enc, password_keychain_id,
                tls_enabled, tls_verify, ca_cert_path, client_cert_path, client_key_path,
                command, now, id,
            ],
        )?;
        if affected == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    })
    .map_err(|e| e.to_string())
}

/// Deletes a proxy and sets network_proxy_id to NULL on servers referencing it.
pub fn delete(db: &Database, id: &str, now: &str) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE servers SET network_proxy_id = NULL, updated_at = ?1 WHERE network_proxy_id = ?2",
            rusqlite::params![now, id],
        )?;
        conn.execute("DELETE FROM proxies WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

/// Counts how many servers reference a given proxy.
pub fn usage_count(db: &Database, proxy_id: &str) -> Result<i32, String> {
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM servers WHERE network_proxy_id = ?1",
            rusqlite::params![proxy_id],
            |row| row.get(0),
        )
    })
    .map_err(|e| e.to_string())
}
