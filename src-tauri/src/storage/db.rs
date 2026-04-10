use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::storage::migrations;

/// Database error types.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Failed to determine data directory")]
    NoDataDir,

    #[error("Failed to create data directory: {0}")]
    CreateDir(std::io::Error),
}

/// Thread-safe database handle wrapping a SQLCipher connection.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Opens (or creates) the database at the default app data path.
    /// When `master_password` is `Some`, the database is encrypted with SQLCipher.
    /// When `None`, the database is opened without encryption.
    pub fn open(master_password: Option<&str>) -> Result<Self, DbError> {
        let path = Self::db_path()?;
        Self::open_at(path, master_password)
    }

    /// Opens (or creates) the database at a specific path.
    pub fn open_at(path: PathBuf, master_password: Option<&str>) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(DbError::CreateDir)?;
        }

        let conn = Connection::open(&path)?;

        // Apply SQLCipher encryption key if provided
        if let Some(password) = master_password {
            conn.pragma_update(None, "key", password)?;
        }

        // Enable WAL mode for better concurrent read performance
        eprintln!(">>> [DB] Setting pragmas...");
        let pragma_start = std::time::Instant::now();
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        eprintln!(">>> [DB] Pragmas set in {}ms", pragma_start.elapsed().as_millis());

        let db = Self {
            conn: Mutex::new(conn),
        };

        // Run migrations to ensure schema is up to date
        eprintln!(">>> [DB] Running migrations...");
        let mig_start = std::time::Instant::now();
        db.migrate()?;
        eprintln!(">>> [DB] Migrations done in {}ms", mig_start.elapsed().as_millis());

        Ok(db)
    }

    /// Executes a closure with an exclusive reference to the connection.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    {
        let conn = self.conn.lock().expect("database mutex poisoned");
        f(&conn).map_err(DbError::Sqlite)
    }

    /// Runs all pending database migrations.
    fn migrate(&self) -> Result<(), DbError> {
        let conn = self.conn.lock().expect("database mutex poisoned");
        migrations::run_migrations(&conn)?;
        Ok(())
    }

    /// Returns the database file path (portable-aware).
    fn db_path() -> Result<PathBuf, DbError> {
        Ok(crate::paths::db_path())
    }
}