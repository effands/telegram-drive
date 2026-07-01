use tauri::{AppHandle, Manager};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type DbConnection = Arc<Mutex<sqlite::Connection>>;

/// Maximum number of retry attempts for database initialization
const MAX_DB_INIT_RETRIES: u32 = 5;

pub fn schema_sql() -> &'static str {
    "CREATE TABLE IF NOT EXISTS shared_links (
        id TEXT PRIMARY KEY,
        folder_id INTEGER,
        message_id INTEGER NOT NULL,
        file_name TEXT NOT NULL,
        file_size INTEGER NOT NULL DEFAULT 0,
        password_hash TEXT,
        password_salt TEXT,
        expires_at INTEGER,
        revoked INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL,
        account_id TEXT
    );
    CREATE TABLE IF NOT EXISTS groups (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        color_hex TEXT DEFAULT '#3B82F6',
        display_order INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS telegram_accounts (
        account_id TEXT PRIMARY KEY,
        display_name TEXT NOT NULL,
        phone TEXT,
        username TEXT,
        session_path TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'active',
        is_default INTEGER NOT NULL DEFAULT 0,
        tracked_bytes INTEGER NOT NULL DEFAULT 0,
        tracked_files INTEGER NOT NULL DEFAULT 0,
        last_sync_at INTEGER,
        last_error TEXT,
        created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
    );
    CREATE TABLE IF NOT EXISTS folder_metadata (
        channel_id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        username TEXT,
        is_public INTEGER NOT NULL DEFAULT 0,
        display_order INTEGER NOT NULL DEFAULT 0,
        group_id INTEGER,
        account_id TEXT,
        locked_account_id TEXT,
        FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE SET NULL,
        FOREIGN KEY(account_id) REFERENCES telegram_accounts(account_id) ON DELETE SET NULL,
        FOREIGN KEY(locked_account_id) REFERENCES telegram_accounts(account_id) ON DELETE SET NULL
    );
    CREATE TABLE IF NOT EXISTS file_accounting (
        account_id TEXT NOT NULL,
        folder_id INTEGER,
        message_id INTEGER NOT NULL,
        file_name TEXT NOT NULL,
        file_size INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        PRIMARY KEY(account_id, folder_id, message_id),
        FOREIGN KEY(account_id) REFERENCES telegram_accounts(account_id) ON DELETE CASCADE
    );"
}

pub fn init_db(app: &AppHandle) -> Result<DbConnection, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db_path = dir.join("shares.db");
    
    // Retry opening the database with exponential backoff.
    // SQLite may report "database is locked" if another process or a stale
    // wal/shm journal hasn't been cleaned up yet (e.g., after a crash).
    let conn = {
        let mut last_err = String::new();
        let mut opened = None;
        for attempt in 0..MAX_DB_INIT_RETRIES {
            match sqlite::open(&db_path) {
                Ok(c) => {
                    opened = Some(c);
                    break;
                }
                Err(e) => {
                    last_err = e.to_string();
                    if attempt < MAX_DB_INIT_RETRIES - 1 {
                        let wait_ms = 100 * 2u64.pow(attempt);
                        log::warn!(
                            "Failed to open SQLite database (attempt {}/{}): {}. Retrying in {}ms...",
                            attempt + 1, MAX_DB_INIT_RETRIES, last_err, wait_ms
                        );
                        std::thread::sleep(Duration::from_millis(wait_ms));
                    }
                }
            }
        }
        opened.ok_or_else(|| {
            format!(
                "Failed to open SQLite database after {} attempts: {}",
                MAX_DB_INIT_RETRIES, last_err
            )
        })?
    };
    
    // Run migration (also with retry for locked-database scenarios)
    {
        let mut last_err = String::new();
        for attempt in 0..MAX_DB_INIT_RETRIES {
            match conn.execute(schema_sql()) {
                Ok(_) => {
                    let _ = conn.execute("ALTER TABLE shared_links ADD COLUMN account_id TEXT");
                    let _ = conn.execute("ALTER TABLE folder_metadata ADD COLUMN account_id TEXT");
                    let _ = conn.execute("ALTER TABLE folder_metadata ADD COLUMN locked_account_id TEXT");
                    last_err.clear();
                    break;
                }
                Err(e) => {
                    last_err = e.to_string();
                    if attempt < MAX_DB_INIT_RETRIES - 1 {
                        let wait_ms = 100 * 2u64.pow(attempt);
                        log::warn!(
                            "Failed to run SQLite migration (attempt {}/{}): {}. Retrying in {}ms...",
                            attempt + 1, MAX_DB_INIT_RETRIES, last_err, wait_ms
                        );
                        std::thread::sleep(Duration::from_millis(wait_ms));
                    }
                }
            }
        }
        if !last_err.is_empty() {
            return Err(format!(
                "Failed to run SQLite migration after {} attempts: {}",
                MAX_DB_INIT_RETRIES, last_err
            ));
        }
    }
    
    log::info!("SQLite database initialized successfully using sqlite crate.");
    Ok(Arc::new(Mutex::new(conn)))
}
