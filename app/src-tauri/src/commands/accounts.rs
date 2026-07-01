use crate::db::DbConnection;
use crate::models::{
    AccountStorageSummary, TelegramAccount, TelegramAccountStatus, UploadRouteDecision,
};
use sqlite::Connection;
use tauri::{AppHandle, Manager, State};

pub const DEFAULT_ACCOUNT_ID: &str = "default";

pub fn current_default_session_path(app: &AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("telegram.session").to_string_lossy().to_string())
}

#[tauri::command]
pub async fn cmd_prepare_new_account_session(
    app_handle: AppHandle,
) -> Result<String, String> {
    let id = format!("acct_{}", chrono::Utc::now().timestamp_millis());
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("sessions")
        .join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn migrate_default_account(conn: &Connection, session_path: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM telegram_accounts")
        .map_err(|e| e.to_string())?;
    let count = match stmt.next().map_err(|e| e.to_string())? {
        sqlite::State::Row => stmt.read::<i64, _>(0).map_err(|e| e.to_string())?,
        sqlite::State::Done => 0,
    };

    if count == 0 {
        let mut stmt = conn.prepare(
            "INSERT INTO telegram_accounts
             (account_id, display_name, session_path, status, is_default, tracked_bytes, tracked_files)
             VALUES (?, ?, ?, ?, 1, 0, 0)"
        ).map_err(|e| e.to_string())?;
        stmt.bind((1, DEFAULT_ACCOUNT_ID)).map_err(|e| e.to_string())?;
        stmt.bind((2, "Main Account")).map_err(|e| e.to_string())?;
        stmt.bind((3, session_path)).map_err(|e| e.to_string())?;
        stmt.bind((4, "active")).map_err(|e| e.to_string())?;
        stmt.next().map_err(|e| e.to_string())?;
    }

    conn.execute("UPDATE folder_metadata SET account_id = 'default' WHERE account_id IS NULL").map_err(|e| e.to_string())?;
    conn.execute("UPDATE shared_links SET account_id = 'default' WHERE account_id IS NULL").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_account_after_login(
    conn: &Connection,
    account_id: &str,
    display_name: &str,
    phone: Option<&str>,
    username: Option<&str>,
    session_path: &str,
) -> Result<(), String> {
    let mut stmt = conn.prepare(
        "INSERT INTO telegram_accounts
         (account_id, display_name, phone, username, session_path, status, is_default)
         VALUES (?, ?, ?, ?, ?, 'active', 0)
         ON CONFLICT(account_id) DO UPDATE SET
           display_name = excluded.display_name,
           phone = excluded.phone,
           username = excluded.username,
           session_path = excluded.session_path,
           status = 'active',
           updated_at = strftime('%s','now')"
    ).map_err(|e| e.to_string())?;
    stmt.bind((1, account_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, display_name)).map_err(|e| e.to_string())?;
    stmt.bind((3, phone)).map_err(|e| e.to_string())?;
    stmt.bind((4, username)).map_err(|e| e.to_string())?;
    stmt.bind((5, session_path)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_accounts_from_db(conn: &Connection) -> Result<Vec<TelegramAccount>, String> {
    let mut stmt = conn.prepare(
        "SELECT account_id, display_name, phone, username, session_path, status,
                is_default, tracked_bytes, tracked_files, last_sync_at, last_error
         FROM telegram_accounts
         ORDER BY is_default DESC, created_at ASC"
    ).map_err(|e| e.to_string())?;

    let mut accounts = Vec::new();
    loop {
        match stmt.next().map_err(|e| e.to_string())? {
            sqlite::State::Row => {
                let status_raw = stmt.read::<String, _>("status").map_err(|e| e.to_string())?;
                let status = match status_raw.as_str() {
                    "offline" => TelegramAccountStatus::Offline,
                    "rate_limited" => TelegramAccountStatus::RateLimited,
                    "needs_login" => TelegramAccountStatus::NeedsLogin,
                    "disabled" => TelegramAccountStatus::Disabled,
                    _ => TelegramAccountStatus::Active,
                };
                accounts.push(TelegramAccount {
                    account_id: stmt.read::<String, _>("account_id").map_err(|e| e.to_string())?,
                    display_name: stmt.read::<String, _>("display_name").map_err(|e| e.to_string())?,
                    phone: stmt.read::<Option<String>, _>("phone").map_err(|e| e.to_string())?,
                    username: stmt.read::<Option<String>, _>("username").map_err(|e| e.to_string())?,
                    session_path: stmt.read::<String, _>("session_path").map_err(|e| e.to_string())?,
                    status,
                    is_default: stmt.read::<i64, _>("is_default").map_err(|e| e.to_string())? == 1,
                    tracked_bytes: stmt.read::<i64, _>("tracked_bytes").map_err(|e| e.to_string())?.max(0) as u64,
                    tracked_files: stmt.read::<i64, _>("tracked_files").map_err(|e| e.to_string())?.max(0) as u64,
                    last_sync_at: stmt.read::<Option<i64>, _>("last_sync_at").map_err(|e| e.to_string())?,
                    last_error: stmt.read::<Option<String>, _>("last_error").map_err(|e| e.to_string())?,
                });
            }
            sqlite::State::Done => break,
        }
    }
    Ok(accounts)
}

pub fn account_summary_from_db(conn: &Connection) -> Result<AccountStorageSummary, String> {
    let accounts = list_accounts_from_db(conn)?;
    let total_bytes = accounts.iter().map(|a| a.tracked_bytes).sum();
    let total_files = accounts.iter().map(|a| a.tracked_files).sum();
    Ok(AccountStorageSummary { total_bytes, total_files, accounts })
}

pub fn set_folder_locked_account_in_db(
    conn: &Connection,
    folder_id: i64,
    account_id: Option<&str>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("UPDATE folder_metadata SET locked_account_id = ? WHERE channel_id = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, account_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, folder_id)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_folder_locked_account_from_db(
    conn: &Connection,
    folder_id: i64,
) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare("SELECT locked_account_id FROM folder_metadata WHERE channel_id = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, folder_id)).map_err(|e| e.to_string())?;

    match stmt.next().map_err(|e| e.to_string())? {
        sqlite::State::Row => stmt.read::<Option<String>, _>(0).map_err(|e| e.to_string()),
        sqlite::State::Done => Ok(None),
    }
}

#[tauri::command]
pub async fn cmd_list_accounts(
    app_handle: AppHandle,
    db_pool: State<'_, DbConnection>,
) -> Result<Vec<TelegramAccount>, String> {
    let session_path = current_default_session_path(&app_handle)?;
    let conn = db_pool.lock().map_err(|_| "DB poisoned".to_string())?;
    migrate_default_account(&conn, &session_path)?;
    list_accounts_from_db(&conn)
}

#[tauri::command]
pub async fn cmd_account_storage_summary(
    app_handle: AppHandle,
    db_pool: State<'_, DbConnection>,
) -> Result<AccountStorageSummary, String> {
    let session_path = current_default_session_path(&app_handle)?;
    let conn = db_pool.lock().map_err(|_| "DB poisoned".to_string())?;
    migrate_default_account(&conn, &session_path)?;
    account_summary_from_db(&conn)
}

#[tauri::command]
pub async fn cmd_set_folder_locked_account(
    folder_id: i64,
    account_id: Option<String>,
    db_pool: State<'_, DbConnection>,
) -> Result<bool, String> {
    let conn = db_pool.lock().map_err(|_| "DB poisoned".to_string())?;
    set_folder_locked_account_in_db(&conn, folder_id, account_id.as_deref())?;
    Ok(true)
}

#[tauri::command]
pub async fn cmd_preview_upload_route(
    app_handle: AppHandle,
    folder_id: Option<i64>,
    db_pool: State<'_, DbConnection>,
) -> Result<UploadRouteDecision, String> {
    let session_path = current_default_session_path(&app_handle)?;
    let conn = db_pool.lock().map_err(|_| "DB poisoned".to_string())?;
    migrate_default_account(&conn, &session_path)?;
    let accounts = list_accounts_from_db(&conn)?;
    let locked = match folder_id {
        Some(id) => get_folder_locked_account_from_db(&conn, id)?,
        None => None,
    };
    Ok(crate::commands::account_router::choose_upload_account(&accounts, locked.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = sqlite::open(":memory:").unwrap();
        conn.execute(crate::db::schema_sql()).unwrap();
        conn
    }

    fn read_string(conn: &Connection, query: &str) -> String {
        let mut stmt = conn.prepare(query).unwrap();
        assert_eq!(stmt.next().unwrap(), sqlite::State::Row);
        stmt.read::<String, _>(0).unwrap()
    }

    #[test]
    fn migrate_default_account_creates_one_default_account() {
        let conn = memory_db();
        migrate_default_account(&conn, "C:/Users/RTX/session/telegram.session").unwrap();

        let accounts = list_accounts_from_db(&conn).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].account_id, DEFAULT_ACCOUNT_ID);
        assert!(accounts[0].is_default);
        assert_eq!(accounts[0].status, TelegramAccountStatus::Active);
    }

    #[test]
    fn account_summary_totals_enabled_accounts() {
        let conn = memory_db();
        migrate_default_account(&conn, "C:/Users/RTX/session/telegram.session").unwrap();
        conn.execute("UPDATE telegram_accounts SET tracked_bytes = 42, tracked_files = 2 WHERE account_id = 'default'").unwrap();

        let summary = account_summary_from_db(&conn).unwrap();
        assert_eq!(summary.total_bytes, 42);
        assert_eq!(summary.total_files, 2);
    }

    #[test]
    fn migrate_default_account_assigns_existing_rows_to_default() {
        let conn = memory_db();
        conn.execute(
            "INSERT INTO folder_metadata (channel_id, name) VALUES (10, 'Videos');
             INSERT INTO shared_links
                (id, folder_id, message_id, file_name, file_size, created_at)
              VALUES ('share-1', 10, 99, 'clip.mp4', 123, 1);"
        ).unwrap();

        migrate_default_account(&conn, "C:/Users/RTX/session/telegram.session").unwrap();

        assert_eq!(
            read_string(&conn, "SELECT account_id FROM folder_metadata WHERE channel_id = 10"),
            DEFAULT_ACCOUNT_ID
        );
        assert_eq!(
            read_string(&conn, "SELECT account_id FROM shared_links WHERE id = 'share-1'"),
            DEFAULT_ACCOUNT_ID
        );
    }

    #[test]
    fn folder_lock_round_trips_locked_account() {
        let conn = memory_db();
        migrate_default_account(&conn, "C:/session/telegram.session").unwrap();
        conn.execute("INSERT INTO folder_metadata (channel_id, name, account_id) VALUES (100, 'Videos', 'default')").unwrap();

        set_folder_locked_account_in_db(&conn, 100, Some("default")).unwrap();
        assert_eq!(
            get_folder_locked_account_from_db(&conn, 100).unwrap().as_deref(),
            Some("default")
        );

        set_folder_locked_account_in_db(&conn, 100, None).unwrap();
        assert_eq!(get_folder_locked_account_from_db(&conn, 100).unwrap(), None);
    }
}
