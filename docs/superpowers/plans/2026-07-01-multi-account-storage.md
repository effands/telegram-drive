# Multi-Account Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add hybrid multi-account Telegram storage management while keeping the existing dashboard as the single primary control screen.

**Architecture:** Introduce a local account registry and account-aware routing layer before changing upload/download behavior. Existing single-account installs migrate into a default account, then operations gradually resolve the correct Telegram client through account-aware helpers. The dashboard gets a compact collapsible right panel for account/storage controls without redesigning the file manager.

**Tech Stack:** Tauri 2, Rust, grammers Telegram client, SQLite via `sqlite`, React 19, TypeScript, Vite, TanStack Query, Tailwind CSS.

---

## Scope Notes

This plan implements the first production-ready slice of the approved design:

- Preserve existing single-account login.
- Migrate current session and folder metadata into a default account.
- Add backend account registry and account-aware upload routing.
- Add account/storage panel on the existing dashboard.
- Add folder lock controls and fallback decision flow.
- Keep one-screen workflow.

The current repository had build dependencies removed to reduce local disk usage. Before executing implementation, restore dependencies with `npm install` inside `app/`.

## File Structure

- `app/src-tauri/src/models.rs`: add account, storage, routing, and fallback DTOs.
- `app/src-tauri/src/db.rs`: create account and file accounting tables; migrate existing rows.
- `app/src-tauri/src/commands/accounts.rs`: new Tauri commands for account list, migration, stats, folder lock, sync state, and routing decisions.
- `app/src-tauri/src/commands/account_router.rs`: pure routing logic and tests.
- `app/src-tauri/src/commands/auth.rs`: add account-aware session path support while preserving current login.
- `app/src-tauri/src/commands/fs.rs`: account-aware upload routing and account fields in file/folder output.
- `app/src-tauri/src/commands/folder_groups.rs`: account fields on enriched folder data.
- `app/src-tauri/src/commands/mod.rs`: export new command modules.
- `app/src-tauri/src/lib.rs`: register account commands and initialize expanded state.
- `app/src/types.ts`: add frontend account and routing types.
- `app/src/hooks/useAccounts.ts`: fetch and mutate account data.
- `app/src/components/desktop/dashboard/StorageAccountsPanel.tsx`: new right panel.
- `app/src/components/desktop/dashboard/FolderAccountLock.tsx`: active folder account lock selector.
- `app/src/components/desktop/dashboard/AccountFallbackDialog.tsx`: prompt when locked account cannot be used.
- `app/src/components/desktop/DesktopDashboard.tsx`: mount the panel and wire fallback decisions.
- `app/src/components/shared/AuthWizard.tsx`: allow account-add mode without replacing current account until login succeeds.
- `app/src/context/SettingsContext.tsx`: remember account panel collapsed state.

## Task 0: Restore Tooling And Create A Working Branch

**Files:**
- No source edits.

- [ ] **Step 1: Restore frontend dependencies**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm install
```

Expected: `node_modules` is recreated and npm exits with code `0`.

- [ ] **Step 2: Create a feature branch**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git checkout -b codex/multi-account-storage
```

Expected: branch changes from `main` to `codex/multi-account-storage`.

- [ ] **Step 3: Verify baseline frontend build**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
```

Expected: TypeScript and Vite build exit code `0`. Chunk size warnings are acceptable.

- [ ] **Step 4: Verify baseline Rust check**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
```

Expected: cargo exits with code `0`.

## Task 1: Add Account And Routing Models

**Files:**
- Modify: `app/src-tauri/src/models.rs`
- Test: `app/src-tauri/src/models.rs`

- [ ] **Step 1: Write model serialization tests**

Append this test module to `app/src-tauri/src/models.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telegram_account_status_serializes_as_snake_case() {
        let status = TelegramAccountStatus::NeedsLogin;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"needs_login\"");
    }

    #[test]
    fn upload_route_serializes_manual_fallback_payload() {
        let route = UploadRouteDecision {
            status: UploadRouteStatus::NeedsUserDecision,
            account_id: None,
            reason: Some("Locked account is offline".to_string()),
            fallback_account_id: Some("acct_b".to_string()),
        };

        let value = serde_json::to_value(route).unwrap();
        assert_eq!(value["status"], "needs_user_decision");
        assert_eq!(value["fallback_account_id"], "acct_b");
    }
}
```

- [ ] **Step 2: Run test and verify RED**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test models::tests -- --nocapture
```

Expected: compile fails because `TelegramAccountStatus`, `UploadRouteDecision`, and `UploadRouteStatus` do not exist.

- [ ] **Step 3: Add model types**

Add these definitions below `FolderGroup` in `app/src-tauri/src/models.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TelegramAccountStatus {
    Active,
    Offline,
    RateLimited,
    NeedsLogin,
    Disabled,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelegramAccount {
    pub account_id: String,
    pub display_name: String,
    pub phone: Option<String>,
    pub username: Option<String>,
    pub session_path: String,
    pub status: TelegramAccountStatus,
    pub is_default: bool,
    pub tracked_bytes: u64,
    pub tracked_files: u64,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountStorageSummary {
    pub total_bytes: u64,
    pub total_files: u64,
    pub accounts: Vec<TelegramAccount>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UploadRouteStatus {
    Ready,
    NeedsUserDecision,
    NoAvailableAccount,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UploadRouteDecision {
    pub status: UploadRouteStatus,
    pub account_id: Option<String>,
    pub reason: Option<String>,
    pub fallback_account_id: Option<String>,
}
```

Extend `FileMetadata` and `FolderMetadata`:

```rust
pub account_id: Option<String>,
```

For `FileMetadata`, place it after `folder_id`. For `FolderMetadata`, place it after `parent_id`.

- [ ] **Step 4: Update existing constructors**

Every `FileMetadata { ... }` literal in `app/src-tauri/src/commands/fs.rs` must include:

```rust
account_id: None,
```

Every `FolderMetadata { ... }` literal in `app/src-tauri/src/commands/fs.rs`, `app/src-tauri/src/commands/folder_groups.rs`, and `app/src-tauri/src/api_routes.rs` must include:

```rust
account_id: None,
```

- [ ] **Step 5: Run test and verify GREEN**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test models::tests -- --nocapture
```

Expected: both model tests pass.

- [ ] **Step 6: Commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/models.rs app/src-tauri/src/commands/fs.rs app/src-tauri/src/commands/folder_groups.rs app/src-tauri/src/api_routes.rs
git commit -m "feat: add account storage models"
```

## Task 2: Add SQLite Account Schema And Migration Helpers

**Files:**
- Modify: `app/src-tauri/src/db.rs`
- Create: `app/src-tauri/src/commands/accounts.rs`
- Modify: `app/src-tauri/src/commands/mod.rs`
- Test: `app/src-tauri/src/commands/accounts.rs`

- [ ] **Step 1: Create failing tests for account migration**

Create `app/src-tauri/src/commands/accounts.rs`:

```rust
use crate::models::{AccountStorageSummary, TelegramAccount, TelegramAccountStatus};
use sqlite::Connection;

pub const DEFAULT_ACCOUNT_ID: &str = "default";

pub fn migrate_default_account(conn: &Connection, session_path: &str) -> Result<(), String> {
    let _ = (conn, session_path);
    Err("account migration not implemented".to_string())
}

pub fn list_accounts_from_db(conn: &Connection) -> Result<Vec<TelegramAccount>, String> {
    let _ = conn;
    Err("account list not implemented".to_string())
}

pub fn account_summary_from_db(conn: &Connection) -> Result<AccountStorageSummary, String> {
    let accounts = list_accounts_from_db(conn)?;
    let total_bytes = accounts.iter().map(|a| a.tracked_bytes).sum();
    let total_files = accounts.iter().map(|a| a.tracked_files).sum();
    Ok(AccountStorageSummary { total_bytes, total_files, accounts })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = sqlite::open(":memory:").unwrap();
        conn.execute(crate::db::schema_sql()).unwrap();
        conn
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
}
```

- [ ] **Step 2: Export module**

Add to `app/src-tauri/src/commands/mod.rs`:

```rust
pub mod accounts;
pub use accounts::*;
```

- [ ] **Step 3: Run tests and verify RED**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test commands::accounts::tests -- --nocapture
```

Expected: compile fails because `crate::db::schema_sql()` does not exist, or tests fail because migration returns an error.

- [ ] **Step 4: Extract schema SQL and add account tables**

In `app/src-tauri/src/db.rs`, add:

```rust
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
```

Replace the inline SQL in `init_db()` with:

```rust
match conn.execute(schema_sql()) {
```

Then add compatibility migration after schema creation:

```rust
let _ = conn.execute("ALTER TABLE shared_links ADD COLUMN account_id TEXT");
let _ = conn.execute("ALTER TABLE folder_metadata ADD COLUMN account_id TEXT");
let _ = conn.execute("ALTER TABLE folder_metadata ADD COLUMN locked_account_id TEXT");
```

- [ ] **Step 5: Implement account DB helpers**

Replace placeholder bodies in `accounts.rs`:

```rust
pub fn migrate_default_account(conn: &Connection, session_path: &str) -> Result<(), String> {
    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM telegram_accounts")
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .and_then(Result::ok)
        .and_then(|row| row.read::<i64, _>(0).into())
        .unwrap_or(0);

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

pub fn list_accounts_from_db(conn: &Connection) -> Result<Vec<TelegramAccount>, String> {
    let mut stmt = conn.prepare(
        "SELECT account_id, display_name, phone, username, session_path, status,
                is_default, tracked_bytes, tracked_files, last_sync_at, last_error
         FROM telegram_accounts
         ORDER BY is_default DESC, created_at ASC"
    ).map_err(|e| e.to_string())?;

    let mut accounts = Vec::new();
    while let Ok(sqlite::State::Row) = stmt.next() {
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
    Ok(accounts)
}
```

- [ ] **Step 6: Run tests and verify GREEN**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test commands::accounts::tests -- --nocapture
```

Expected: both account migration tests pass.

- [ ] **Step 7: Commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/db.rs app/src-tauri/src/commands/accounts.rs app/src-tauri/src/commands/mod.rs
git commit -m "feat: add account registry schema"
```

## Task 3: Add Pure Upload Router

**Files:**
- Create: `app/src-tauri/src/commands/account_router.rs`
- Modify: `app/src-tauri/src/commands/mod.rs`
- Test: `app/src-tauri/src/commands/account_router.rs`

- [ ] **Step 1: Write failing router tests**

Create `app/src-tauri/src/commands/account_router.rs`:

```rust
use crate::models::{TelegramAccount, TelegramAccountStatus, UploadRouteDecision, UploadRouteStatus};

pub fn choose_upload_account(
    accounts: &[TelegramAccount],
    locked_account_id: Option<&str>,
) -> UploadRouteDecision {
    let _ = (accounts, locked_account_id);
    UploadRouteDecision {
        status: UploadRouteStatus::NoAvailableAccount,
        account_id: None,
        reason: Some("router not implemented".to_string()),
        fallback_account_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str, status: TelegramAccountStatus, bytes: u64) -> TelegramAccount {
        TelegramAccount {
            account_id: id.to_string(),
            display_name: id.to_string(),
            phone: None,
            username: None,
            session_path: format!("sessions/{id}/telegram.session"),
            status,
            is_default: id == "a",
            tracked_bytes: bytes,
            tracked_files: 0,
            last_sync_at: None,
            last_error: None,
        }
    }

    #[test]
    fn unlocked_folder_chooses_active_account_with_lowest_tracked_bytes() {
        let accounts = vec![
            account("a", TelegramAccountStatus::Active, 500),
            account("b", TelegramAccountStatus::Active, 100),
        ];

        let decision = choose_upload_account(&accounts, None);
        assert_eq!(decision.status, UploadRouteStatus::Ready);
        assert_eq!(decision.account_id.as_deref(), Some("b"));
    }

    #[test]
    fn locked_folder_uses_locked_account_when_active() {
        let accounts = vec![
            account("a", TelegramAccountStatus::Active, 500),
            account("b", TelegramAccountStatus::Active, 100),
        ];

        let decision = choose_upload_account(&accounts, Some("a"));
        assert_eq!(decision.status, UploadRouteStatus::Ready);
        assert_eq!(decision.account_id.as_deref(), Some("a"));
    }

    #[test]
    fn locked_folder_requires_user_decision_when_locked_account_is_offline() {
        let accounts = vec![
            account("a", TelegramAccountStatus::Offline, 500),
            account("b", TelegramAccountStatus::Active, 100),
        ];

        let decision = choose_upload_account(&accounts, Some("a"));
        assert_eq!(decision.status, UploadRouteStatus::NeedsUserDecision);
        assert_eq!(decision.account_id, None);
        assert_eq!(decision.fallback_account_id.as_deref(), Some("b"));
    }
}
```

- [ ] **Step 2: Export module and run RED**

Add to `app/src-tauri/src/commands/mod.rs`:

```rust
pub mod account_router;
pub use account_router::*;
```

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test commands::account_router::tests -- --nocapture
```

Expected: two tests fail because the router always returns `NoAvailableAccount`.

- [ ] **Step 3: Implement router**

Replace `choose_upload_account` with:

```rust
pub fn choose_upload_account(
    accounts: &[TelegramAccount],
    locked_account_id: Option<&str>,
) -> UploadRouteDecision {
    let active_accounts: Vec<&TelegramAccount> = accounts
        .iter()
        .filter(|a| a.status == TelegramAccountStatus::Active)
        .collect();

    if let Some(locked_id) = locked_account_id {
        if let Some(locked) = accounts.iter().find(|a| a.account_id == locked_id) {
            if locked.status == TelegramAccountStatus::Active {
                return UploadRouteDecision {
                    status: UploadRouteStatus::Ready,
                    account_id: Some(locked.account_id.clone()),
                    reason: None,
                    fallback_account_id: None,
                };
            }

            let fallback = active_accounts
                .iter()
                .min_by_key(|a| a.tracked_bytes)
                .map(|a| a.account_id.clone());

            return UploadRouteDecision {
                status: UploadRouteStatus::NeedsUserDecision,
                account_id: None,
                reason: Some(format!("Locked account '{}' is {:?}", locked_id, locked.status)),
                fallback_account_id: fallback,
            };
        }

        return UploadRouteDecision {
            status: UploadRouteStatus::NeedsUserDecision,
            account_id: None,
            reason: Some(format!("Locked account '{}' was not found", locked_id)),
            fallback_account_id: active_accounts
                .iter()
                .min_by_key(|a| a.tracked_bytes)
                .map(|a| a.account_id.clone()),
        };
    }

    match active_accounts.iter().min_by_key(|a| a.tracked_bytes) {
        Some(account) => UploadRouteDecision {
            status: UploadRouteStatus::Ready,
            account_id: Some(account.account_id.clone()),
            reason: None,
            fallback_account_id: None,
        },
        None => UploadRouteDecision {
            status: UploadRouteStatus::NoAvailableAccount,
            account_id: None,
            reason: Some("No active Telegram account is available".to_string()),
            fallback_account_id: None,
        },
    }
}
```

- [ ] **Step 4: Run GREEN**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test commands::account_router::tests -- --nocapture
```

Expected: all router tests pass.

- [ ] **Step 5: Commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/account_router.rs app/src-tauri/src/commands/mod.rs
git commit -m "feat: add hybrid upload router"
```

## Task 4: Initialize Default Account During App Startup

**Files:**
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/src-tauri/src/commands/accounts.rs`
- Test: `app/src-tauri/src/commands/accounts.rs`

- [ ] **Step 1: Add command-level helpers**

Add to `accounts.rs`:

```rust
use crate::db::DbConnection;
use tauri::{AppHandle, Manager, State};

pub fn current_default_session_path(app: &AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("telegram.session").to_string_lossy().to_string())
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
```

- [ ] **Step 2: Register commands**

In `app/src-tauri/src/lib.rs`, add to `tauri::generate_handler![...]`:

```rust
commands::cmd_list_accounts,
commands::cmd_account_storage_summary,
```

- [ ] **Step 3: Run Rust check**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
```

Expected: exits with code `0`.

- [ ] **Step 4: Add manual command smoke test**

Run app in dev mode:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run tauri dev
```

Open DevTools console and run:

```javascript
window.__TAURI__.core.invoke("cmd_list_accounts").then(console.log)
```

Expected: returns at least one default account with `account_id: "default"`.

- [ ] **Step 5: Commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/accounts.rs app/src-tauri/src/lib.rs
git commit -m "feat: expose account summary commands"
```

## Task 5: Add Frontend Account Types And Hook

**Files:**
- Modify: `app/src/types.ts`
- Create: `app/src/hooks/useAccounts.ts`
- Test: `app/src/hooks/useAccounts.ts`

- [ ] **Step 1: Add TypeScript types**

Append to `app/src/types.ts`:

```ts
export type TelegramAccountStatus =
  | 'active'
  | 'offline'
  | 'rate_limited'
  | 'needs_login'
  | 'disabled';

export interface TelegramAccount {
  account_id: string;
  display_name: string;
  phone?: string | null;
  username?: string | null;
  session_path: string;
  status: TelegramAccountStatus;
  is_default: boolean;
  tracked_bytes: number;
  tracked_files: number;
  last_sync_at?: number | null;
  last_error?: string | null;
}

export interface AccountStorageSummary {
  total_bytes: number;
  total_files: number;
  accounts: TelegramAccount[];
}

export type UploadRouteStatus = 'ready' | 'needs_user_decision' | 'no_available_account';

export interface UploadRouteDecision {
  status: UploadRouteStatus;
  account_id?: string | null;
  reason?: string | null;
  fallback_account_id?: string | null;
}
```

Add optional account ownership fields:

```ts
account_id?: string | null;
locked_account_id?: string | null;
```

Add them to `TelegramFile` and `TelegramFolder`.

- [ ] **Step 2: Create hook**

Create `app/src/hooks/useAccounts.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import type { AccountStorageSummary, TelegramAccount } from '../types';

export function useAccounts() {
  return useQuery({
    queryKey: ['telegram-accounts'],
    queryFn: () => invoke<TelegramAccount[]>('cmd_list_accounts'),
  });
}

export function useAccountStorageSummary() {
  return useQuery({
    queryKey: ['account-storage-summary'],
    queryFn: () => invoke<AccountStorageSummary>('cmd_account_storage_summary'),
  });
}

export function useRefreshAccountData() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      await queryClient.invalidateQueries({ queryKey: ['telegram-accounts'] });
      await queryClient.invalidateQueries({ queryKey: ['account-storage-summary'] });
    },
  });
}
```

- [ ] **Step 3: Run frontend build**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
```

Expected: TypeScript build exits with code `0`.

- [ ] **Step 4: Commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git add app/src/types.ts app/src/hooks/useAccounts.ts
git commit -m "feat: add frontend account data hook"
```

## Task 6: Add Storage Accounts Panel UI

**Files:**
- Create: `app/src/components/desktop/dashboard/StorageAccountsPanel.tsx`
- Modify: `app/src/components/desktop/DesktopDashboard.tsx`
- Modify: `app/src/context/SettingsContext.tsx`

- [ ] **Step 1: Add setting for panel collapse**

In `SettingsContext.tsx`, add to settings type:

```ts
showAccountPanel: boolean;
```

Add to default settings:

```ts
showAccountPanel: true,
```

- [ ] **Step 2: Create panel component**

Create `app/src/components/desktop/dashboard/StorageAccountsPanel.tsx`:

```tsx
import { Database, Plus, RefreshCw, ShieldCheck, AlertTriangle } from 'lucide-react';
import { useAccountStorageSummary } from '../../../hooks/useAccounts';
import { formatBytes } from '../../../lib/format';

interface StorageAccountsPanelProps {
  onAddAccount: () => void;
}

function statusTone(status: string) {
  if (status === 'active') return 'text-emerald-400 bg-emerald-500/10';
  if (status === 'needs_login' || status === 'rate_limited') return 'text-amber-400 bg-amber-500/10';
  return 'text-rose-400 bg-rose-500/10';
}

export function StorageAccountsPanel({ onAddAccount }: StorageAccountsPanelProps) {
  const { data, isLoading, refetch } = useAccountStorageSummary();
  const accounts = data?.accounts ?? [];

  return (
    <aside className="hidden xl:flex w-80 shrink-0 border-l border-telegram-border bg-telegram-surface/50 flex-col">
      <div className="p-4 border-b border-telegram-border flex items-center justify-between">
        <div>
          <p className="text-xs uppercase tracking-wide text-telegram-subtext font-semibold">Storage Accounts</p>
          <h2 className="text-lg font-semibold text-telegram-text">{formatBytes(data?.total_bytes ?? 0)}</h2>
        </div>
        <button
          type="button"
          onClick={() => refetch()}
          className="p-2 rounded-lg hover:bg-telegram-hover text-telegram-subtext hover:text-telegram-text"
          title="Sync account summary"
        >
          <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
        </button>
      </div>

      <div className="p-3 space-y-2 overflow-y-auto">
        {accounts.map((account) => (
          <div key={account.account_id} className="rounded-lg border border-telegram-border bg-telegram-bg/60 p-3">
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <ShieldCheck className="w-4 h-4 text-telegram-primary" />
                  <p className="font-medium text-telegram-text truncate">{account.display_name}</p>
                </div>
                <p className="text-xs text-telegram-subtext truncate">
                  {account.username ? `@${account.username}` : account.phone || account.account_id}
                </p>
              </div>
              <span className={`text-[10px] px-2 py-1 rounded-full font-semibold ${statusTone(account.status)}`}>
                {account.status.replace('_', ' ')}
              </span>
            </div>
            <div className="mt-3 grid grid-cols-2 gap-2 text-xs">
              <div className="rounded bg-telegram-hover/40 p-2">
                <p className="text-telegram-subtext">Stored</p>
                <p className="text-telegram-text font-semibold">{formatBytes(account.tracked_bytes)}</p>
              </div>
              <div className="rounded bg-telegram-hover/40 p-2">
                <p className="text-telegram-subtext">Files</p>
                <p className="text-telegram-text font-semibold">{account.tracked_files}</p>
              </div>
            </div>
            {account.last_error && (
              <div className="mt-2 flex gap-2 text-xs text-amber-300">
                <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                <span className="line-clamp-2">{account.last_error}</span>
              </div>
            )}
          </div>
        ))}

        <button
          type="button"
          onClick={onAddAccount}
          className="w-full rounded-lg border border-dashed border-telegram-border p-3 text-sm text-telegram-subtext hover:text-telegram-text hover:bg-telegram-hover flex items-center justify-center gap-2"
        >
          <Plus className="w-4 h-4" />
          Add Account
        </button>

        {accounts.length === 0 && !isLoading && (
          <div className="rounded-lg border border-telegram-border p-4 text-center text-sm text-telegram-subtext">
            <Database className="w-6 h-6 mx-auto mb-2" />
            No account records found.
          </div>
        )}
      </div>
    </aside>
  );
}
```

If `app/src/lib/format.ts` does not export `formatBytes`, use the existing formatter currently used for file sizes. If no shared formatter exists, add:

```ts
export function formatBytes(bytes: number): string {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, index)).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}
```

- [ ] **Step 3: Mount panel in dashboard**

In `DesktopDashboard.tsx`, import:

```ts
import { StorageAccountsPanel } from './dashboard/StorageAccountsPanel';
```

Inside the root flex layout, place the panel as the last child:

```tsx
<StorageAccountsPanel onAddAccount={() => setShowAddAccountWizard(true)} />
```

Add local state near other modal states:

```ts
const [showAddAccountWizard, setShowAddAccountWizard] = useState(false);
```

For this task, the `onAddAccount` click can show a toast:

```ts
toast.info('Add Account flow is coming in the next step');
```

Task 9 replaces this with the real account-add wizard.

- [ ] **Step 4: Run frontend build**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
```

Expected: TypeScript build exits with code `0`.

- [ ] **Step 5: Manual visual check**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run tauri dev
```

Expected: dashboard opens with a compact right panel; file manager remains usable and visually familiar.

- [ ] **Step 6: Commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git add app/src/components/desktop/dashboard/StorageAccountsPanel.tsx app/src/components/desktop/DesktopDashboard.tsx app/src/context/SettingsContext.tsx app/src/lib/format.ts
git commit -m "feat: add storage accounts panel"
```

## Task 7: Add Folder Lock Commands And Selector

**Files:**
- Modify: `app/src-tauri/src/commands/accounts.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Create: `app/src/components/desktop/dashboard/FolderAccountLock.tsx`
- Modify: `app/src/components/desktop/DesktopDashboard.tsx`
- Test: `app/src-tauri/src/commands/accounts.rs`

- [ ] **Step 1: Add failing folder lock test**

Append to `accounts.rs` test module:

```rust
#[test]
fn folder_lock_round_trips_locked_account() {
    let conn = memory_db();
    migrate_default_account(&conn, "C:/session/telegram.session").unwrap();
    conn.execute("INSERT INTO folder_metadata (channel_id, name, account_id) VALUES (100, 'Videos', 'default')").unwrap();

    set_folder_locked_account_in_db(&conn, 100, Some("default")).unwrap();
    assert_eq!(get_folder_locked_account_from_db(&conn, 100).unwrap().as_deref(), Some("default"));

    set_folder_locked_account_in_db(&conn, 100, None).unwrap();
    assert_eq!(get_folder_locked_account_from_db(&conn, 100).unwrap(), None);
}
```

- [ ] **Step 2: Run RED**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test commands::accounts::tests::folder_lock_round_trips_locked_account -- --nocapture
```

Expected: compile fails because lock helper functions do not exist.

- [ ] **Step 3: Implement folder lock helpers and commands**

Add to `accounts.rs`:

```rust
pub fn set_folder_locked_account_in_db(
    conn: &Connection,
    folder_id: i64,
    account_id: Option<&str>,
) -> Result<(), String> {
    let mut stmt = conn.prepare(
        "UPDATE folder_metadata SET locked_account_id = ? WHERE channel_id = ?"
    ).map_err(|e| e.to_string())?;
    stmt.bind((1, account_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, folder_id)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_folder_locked_account_from_db(
    conn: &Connection,
    folder_id: i64,
) -> Result<Option<String>, String> {
    let mut stmt = conn.prepare(
        "SELECT locked_account_id FROM folder_metadata WHERE channel_id = ?"
    ).map_err(|e| e.to_string())?;
    stmt.bind((1, folder_id)).map_err(|e| e.to_string())?;
    if let Ok(sqlite::State::Row) = stmt.next() {
        return stmt.read::<Option<String>, _>(0).map_err(|e| e.to_string());
    }
    Ok(None)
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
```

Register in `lib.rs`:

```rust
commands::cmd_set_folder_locked_account,
```

- [ ] **Step 4: Run GREEN**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test commands::accounts::tests::folder_lock_round_trips_locked_account -- --nocapture
```

Expected: test passes.

- [ ] **Step 5: Create selector UI**

Create `app/src/components/desktop/dashboard/FolderAccountLock.tsx`:

```tsx
import { Lock, Unlock } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import type { TelegramAccount, TelegramFolder } from '../../../types';

interface FolderAccountLockProps {
  folder: TelegramFolder | null;
  accounts: TelegramAccount[];
  onChanged: () => void;
}

export function FolderAccountLock({ folder, accounts, onChanged }: FolderAccountLockProps) {
  if (!folder) return null;

  const lockedAccountId = folder.locked_account_id ?? '';

  return (
    <div className="rounded-lg border border-telegram-border bg-telegram-bg/60 p-3">
      <div className="flex items-center gap-2 mb-2">
        {lockedAccountId ? <Lock className="w-4 h-4 text-amber-400" /> : <Unlock className="w-4 h-4 text-telegram-subtext" />}
        <p className="text-sm font-medium text-telegram-text">Folder Upload Account</p>
      </div>
      <select
        value={lockedAccountId}
        onChange={async (event) => {
          const next = event.target.value || null;
          await invoke('cmd_set_folder_locked_account', {
            folderId: folder.id,
            accountId: next,
          });
          toast.success(next ? 'Folder locked to account' : 'Folder uses auto pool');
          onChanged();
        }}
        className="w-full bg-telegram-surface border border-telegram-border rounded px-2 py-2 text-sm text-telegram-text"
      >
        <option value="">Auto Pool</option>
        {accounts.map((account) => (
          <option key={account.account_id} value={account.account_id}>
            {account.display_name}
          </option>
        ))}
      </select>
    </div>
  );
}
```

- [ ] **Step 6: Render selector in account panel**

Pass active folder and accounts into `StorageAccountsPanel`, then render `FolderAccountLock` below the summary.

Expected data flow:

```tsx
<FolderAccountLock
  folder={activeFolder}
  accounts={accounts}
  onChanged={() => {
    refetch();
    queryClient.invalidateQueries({ queryKey: ['files'] });
  }}
/>
```

- [ ] **Step 7: Run build and commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/accounts.rs app/src-tauri/src/lib.rs app/src/components/desktop/dashboard/FolderAccountLock.tsx app/src/components/desktop/dashboard/StorageAccountsPanel.tsx app/src/components/desktop/DesktopDashboard.tsx
git commit -m "feat: add folder account lock control"
```

Expected: build/check pass and commit succeeds.

## Task 8: Route Uploads Through Account Decisions

**Files:**
- Modify: `app/src-tauri/src/commands/fs.rs`
- Modify: `app/src-tauri/src/commands/accounts.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/src/hooks/useFileUpload.ts`
- Create: `app/src/components/desktop/dashboard/AccountFallbackDialog.tsx`
- Modify: `app/src/components/desktop/DesktopDashboard.tsx`

- [ ] **Step 1: Add backend route preview command**

Add to `accounts.rs`:

```rust
#[tauri::command]
pub async fn cmd_preview_upload_route(
    folder_id: Option<i64>,
    db_pool: State<'_, DbConnection>,
) -> Result<UploadRouteDecision, String> {
    let conn = db_pool.lock().map_err(|_| "DB poisoned".to_string())?;
    let accounts = list_accounts_from_db(&conn)?;
    let locked = match folder_id {
        Some(id) => get_folder_locked_account_from_db(&conn, id)?,
        None => None,
    };
    Ok(crate::commands::account_router::choose_upload_account(&accounts, locked.as_deref()))
}
```

Register in `lib.rs`:

```rust
commands::cmd_preview_upload_route,
```

- [ ] **Step 2: Add optional account ID to upload command signature**

In `cmd_upload_file`, `cmd_upload_file_inner`, and `initiate_upload`, add:

```rust
account_id: Option<String>,
```

For now, keep using `state.client` for Telegram client if `account_id` is `None` or `"default"`. If a non-default `account_id` arrives before Task 10 account pool is complete, return:

```rust
return Err("Additional account sessions are not connected yet".to_string());
```

This preserves single-account behavior while making the frontend route-aware.

- [ ] **Step 3: Update frontend queue item**

In `app/src/types.ts`, add to `QueueItem`:

```ts
accountId?: string | null;
routeDecision?: UploadRouteDecision;
```

- [ ] **Step 4: Preview route before queueing uploads**

In `useFileUpload.ts`, update `queueFiles` to:

```ts
const route = await invoke<UploadRouteDecision>('cmd_preview_upload_route', {
  folderId: activeFolderId,
});
```

If `route.status === 'ready'`, add `accountId: route.account_id`.

If `route.status === 'needs_user_decision'`, do not queue immediately. Call a callback from `DesktopDashboard` to show `AccountFallbackDialog`.

Refactor hook signature:

```ts
export function useFileUpload(
  activeFolderId: number | null,
  store: Store | null,
  onRouteDecisionNeeded?: (decision: UploadRouteDecision, paths: string[]) => void,
)
```

- [ ] **Step 5: Pass account ID to upload command**

In `processItem`, change invoke payload:

```ts
await invoke('cmd_upload_file', {
  path: item.path,
  folderId: item.folderId,
  transferId: item.id,
  accountId: item.accountId ?? null,
});
```

- [ ] **Step 6: Create fallback dialog**

Create `app/src/components/desktop/dashboard/AccountFallbackDialog.tsx`:

```tsx
import type { UploadRouteDecision } from '../../../types';

interface AccountFallbackDialogProps {
  decision: UploadRouteDecision;
  count: number;
  onRetry: () => void;
  onUseFallback: () => void;
  onCancel: () => void;
}

export function AccountFallbackDialog({ decision, count, onRetry, onUseFallback, onCancel }: AccountFallbackDialogProps) {
  return (
    <div className="fixed inset-0 z-[260] flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="w-full max-w-md rounded-lg border border-telegram-border bg-telegram-surface p-5 shadow-2xl">
        <h2 className="text-lg font-semibold text-telegram-text">Choose Upload Route</h2>
        <p className="mt-2 text-sm text-telegram-subtext">
          {decision.reason || 'The locked account cannot accept this upload right now.'}
        </p>
        <p className="mt-2 text-sm text-telegram-subtext">
          {count} file{count === 1 ? '' : 's'} are waiting.
        </p>
        <div className="mt-5 flex justify-end gap-2">
          <button onClick={onCancel} className="px-3 py-2 rounded bg-telegram-hover text-telegram-text">Cancel</button>
          <button onClick={onRetry} className="px-3 py-2 rounded bg-telegram-hover text-telegram-text">Retry</button>
          <button
            onClick={onUseFallback}
            disabled={!decision.fallback_account_id}
            className="px-3 py-2 rounded bg-telegram-primary text-black font-semibold disabled:opacity-50"
          >
            Use Fallback
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 7: Wire dialog in dashboard**

In `DesktopDashboard.tsx`, store pending decision:

```ts
const [pendingRouteDecision, setPendingRouteDecision] = useState<{
  decision: UploadRouteDecision;
  paths: string[];
} | null>(null);
```

Pass callback to `useFileUpload`.

When `Use Fallback` is clicked, queue files with `accountId: decision.fallback_account_id`. Expose a hook helper:

```ts
queueFilesWithAccount(paths, decision.fallback_account_id)
```

- [ ] **Step 8: Run checks and commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/fs.rs app/src-tauri/src/commands/accounts.rs app/src-tauri/src/lib.rs app/src/hooks/useFileUpload.ts app/src/types.ts app/src/components/desktop/dashboard/AccountFallbackDialog.tsx app/src/components/desktop/DesktopDashboard.tsx
git commit -m "feat: route uploads through account decisions"
```

Expected: checks pass and commit succeeds.

## Task 9: Add Account-Add Login Mode

**Files:**
- Modify: `app/src/components/shared/AuthWizard.tsx`
- Modify: `app/src/components/desktop/DesktopDashboard.tsx`
- Modify: `app/src-tauri/src/commands/auth.rs`
- Modify: `app/src-tauri/src/commands/accounts.rs`

- [ ] **Step 1: Add account-add props to AuthWizard**

Update props:

```ts
interface AuthWizardProps {
  onLogin: () => void;
  mode?: 'primary-login' | 'add-account';
  onCancel?: () => void;
}
```

Default:

```ts
export function AuthWizard({ onLogin, mode = 'primary-login', onCancel }: AuthWizardProps) {
```

When `mode === 'add-account'`, title should be `Add Telegram Account`, and successful login should call `onLogin` without replacing app auth state.

- [ ] **Step 2: Backend generate account ID for add mode**

Add command to `accounts.rs`:

```rust
#[tauri::command]
pub async fn cmd_prepare_new_account_session(
    app_handle: AppHandle,
) -> Result<String, String> {
    let id = format!("acct_{}", chrono::Utc::now().timestamp_millis());
    let dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?
        .join("sessions")
        .join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(id)
}
```

Register command in `lib.rs`.

- [ ] **Step 3: Extend auth commands with optional account ID**

Add optional `account_id: Option<String>` to:

- `cmd_auth_request_code`
- `cmd_auth_qr_login`

Inside `ensure_client_initialized`, add a sibling helper:

```rust
fn session_path_for_account(app_handle: &tauri::AppHandle, account_id: Option<&str>) -> Result<std::path::PathBuf, String> {
    let app_data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    match account_id {
        Some(id) if id != DEFAULT_ACCOUNT_ID => Ok(app_data_dir.join("sessions").join(id).join("telegram.session")),
        _ => Ok(app_data_dir.join("telegram.session")),
    }
}
```

Use this helper instead of hardcoding `telegram.session`.

- [ ] **Step 4: Persist new account after successful login**

After `client.sign_in` or QR authorization succeeds, call a new helper:

```rust
pub fn upsert_account_after_login(
    conn: &Connection,
    account_id: &str,
    display_name: &str,
    session_path: &str,
) -> Result<(), String> {
    let mut stmt = conn.prepare(
        "INSERT INTO telegram_accounts
         (account_id, display_name, session_path, status, is_default)
         VALUES (?, ?, ?, 'active', 0)
         ON CONFLICT(account_id) DO UPDATE SET
           display_name = excluded.display_name,
           session_path = excluded.session_path,
           status = 'active',
           updated_at = strftime('%s','now')"
    ).map_err(|e| e.to_string())?;
    stmt.bind((1, account_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, display_name)).map_err(|e| e.to_string())?;
    stmt.bind((3, session_path)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}
```

Use `client.get_me().await` to build `display_name`.

- [ ] **Step 5: Mount add-account wizard as modal**

In `DesktopDashboard.tsx`, when `showAddAccountWizard` is true:

```tsx
<div className="fixed inset-0 z-[300] bg-black/70">
  <AuthWizard
    mode="add-account"
    onCancel={() => setShowAddAccountWizard(false)}
    onLogin={() => {
      setShowAddAccountWizard(false);
      queryClient.invalidateQueries({ queryKey: ['telegram-accounts'] });
      queryClient.invalidateQueries({ queryKey: ['account-storage-summary'] });
    }}
  />
</div>
```

- [ ] **Step 6: Run checks and commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
cd "E:\AUTO KLIK\Teledrive"
git add app/src/components/shared/AuthWizard.tsx app/src/components/desktop/DesktopDashboard.tsx app/src-tauri/src/commands/auth.rs app/src-tauri/src/commands/accounts.rs app/src-tauri/src/lib.rs
git commit -m "feat: add secondary account login flow"
```

Expected: checks pass and commit succeeds.

## Task 10: Add Multi-Client Account Pool

**Files:**
- Modify: `app/src-tauri/src/commands/mod.rs`
- Modify: `app/src-tauri/src/commands/auth.rs`
- Modify: `app/src-tauri/src/commands/fs.rs`
- Test: `app/src-tauri/src/commands/account_router.rs`

- [ ] **Step 1: Expand state**

In `TelegramState`, add:

```rust
pub account_clients: Arc<Mutex<HashMap<String, Client>>>,
pub account_runner_shutdowns: Arc<std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
pub account_peer_cache: Arc<tokio::sync::RwLock<HashMap<String, HashMap<i64, Peer>>>>,
```

Initialize in `lib.rs` next to existing state:

```rust
account_clients: Arc::new(Mutex::new(HashMap::new())),
account_runner_shutdowns: Arc::new(std::sync::Mutex::new(HashMap::new())),
account_peer_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
```

- [ ] **Step 2: Add account-aware client initializer**

In `auth.rs`, create:

```rust
pub async fn ensure_account_client_initialized(
    app_handle: &tauri::AppHandle,
    state: &State<'_, TelegramState>,
    account_id: &str,
    api_id: i32,
) -> Result<Client, String> {
    if account_id == DEFAULT_ACCOUNT_ID {
        return ensure_client_initialized(app_handle, state, api_id).await;
    }

    let mut clients = state.account_clients.lock().await;
    if let Some(client) = clients.get(account_id) {
        return Ok(client.clone());
    }
    drop(clients);

    let session_path = session_path_for_account(app_handle, Some(account_id))?;
    let session_path_str = session_path.to_string_lossy().to_string();
    let session = SqliteSession::open(&session_path_str).map_err(|e| e.to_string())?;
    let session = Arc::new(session);
    let pool = SenderPool::with_configuration(session, api_id, grammers_mtsender::ConnectionParams::default());
    let client = Client::new(&pool);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    state.account_runner_shutdowns.lock().unwrap().insert(account_id.to_string(), shutdown_tx);
    let SenderPool { runner, .. } = pool;
    let runner_account_id = account_id.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::select! {
            _ = runner.run() => log::info!("Account runner {} exited", runner_account_id),
            _ = shutdown_rx => log::info!("Account runner {} shutdown requested", runner_account_id),
        }
    });
    state.account_clients.lock().await.insert(account_id.to_string(), client.clone());
    Ok(client)
}
```

- [ ] **Step 3: Resolve client by routed account**

In `fs.rs`, before upload, after route decision:

```rust
let resolved_account_id = account_id.unwrap_or_else(|| DEFAULT_ACCOUNT_ID.to_string());
let client = if resolved_account_id == DEFAULT_ACCOUNT_ID {
    state.client.lock().await.clone().ok_or_else(|| "Client not connected".to_string())?
} else {
    let api_id = state.api_id.lock().await.ok_or_else(|| "No API ID configured".to_string())?;
    crate::commands::auth::ensure_account_client_initialized(&app_handle, &state, &resolved_account_id, api_id).await?
};
```

Use account-specific peer cache for non-default accounts. Add helper:

```rust
async fn resolve_account_peer(
    client: &Client,
    folder_id: Option<i64>,
    state: &TelegramState,
    account_id: &str,
) -> Result<Peer, String> {
    if account_id == DEFAULT_ACCOUNT_ID {
        return resolve_peer(client, folder_id, &state.peer_cache).await;
    }
    let cache_arc = state.account_peer_cache.clone();
    {
        let all = cache_arc.read().await;
        if let Some(cache) = all.get(account_id) {
            if let Some(id) = folder_id {
                if let Some(peer) = cache.get(&id) {
                    return Ok(peer.clone());
                }
            }
        }
    }
    resolve_peer(client, folder_id, &state.peer_cache).await
}
```

Use this helper for upload first; migrate download/preview/delete/move in Task 11.

- [ ] **Step 4: Run checks and commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/mod.rs app/src-tauri/src/commands/auth.rs app/src-tauri/src/commands/fs.rs app/src-tauri/src/lib.rs
git commit -m "feat: add telegram account client pool"
```

Expected: cargo check passes and commit succeeds.

## Task 11: Account-Aware File Metadata And Storage Accounting

**Files:**
- Modify: `app/src-tauri/src/commands/fs.rs`
- Modify: `app/src-tauri/src/commands/accounts.rs`
- Modify: `app/src-tauri/src/commands/folder_groups.rs`
- Modify: `app/src/types.ts`

- [ ] **Step 1: Record uploaded files in `file_accounting`**

After successful `send_message`, capture returned message ID:

```rust
Ok(sent) => {
    record_file_accounting(
        &db_conn,
        &resolved_account_id,
        folder_id,
        sent.id() as i64,
        &file_name_for_record,
        size,
    )?;
}
```

Add helper in `accounts.rs`:

```rust
pub fn record_file_accounting(
    conn: &Connection,
    account_id: &str,
    folder_id: Option<i64>,
    message_id: i64,
    file_name: &str,
    file_size: u64,
) -> Result<(), String> {
    let mut stmt = conn.prepare(
        "INSERT INTO file_accounting (account_id, folder_id, message_id, file_name, file_size)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(account_id, folder_id, message_id) DO UPDATE SET
           file_name = excluded.file_name,
           file_size = excluded.file_size,
           updated_at = strftime('%s','now')"
    ).map_err(|e| e.to_string())?;
    stmt.bind((1, account_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, folder_id)).map_err(|e| e.to_string())?;
    stmt.bind((3, message_id)).map_err(|e| e.to_string())?;
    stmt.bind((4, file_name)).map_err(|e| e.to_string())?;
    stmt.bind((5, file_size as i64)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    refresh_account_totals(conn, account_id)?;
    Ok(())
}

pub fn refresh_account_totals(conn: &Connection, account_id: &str) -> Result<(), String> {
    let query = "SELECT COALESCE(SUM(file_size), 0), COUNT(*) FROM file_accounting WHERE account_id = ?";
    let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;
    stmt.bind((1, account_id)).map_err(|e| e.to_string())?;
    let (bytes, files) = if let Ok(sqlite::State::Row) = stmt.next() {
        (
            stmt.read::<i64, _>(0).map_err(|e| e.to_string())?,
            stmt.read::<i64, _>(1).map_err(|e| e.to_string())?,
        )
    } else {
        (0, 0)
    };
    let mut update = conn.prepare("UPDATE telegram_accounts SET tracked_bytes = ?, tracked_files = ?, updated_at = strftime('%s','now') WHERE account_id = ?").map_err(|e| e.to_string())?;
    update.bind((1, bytes)).map_err(|e| e.to_string())?;
    update.bind((2, files)).map_err(|e| e.to_string())?;
    update.bind((3, account_id)).map_err(|e| e.to_string())?;
    update.next().map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: Set account ID on folder and file DTOs**

When scanning folders for a specific account, set:

```rust
account_id: Some(resolved_account_id.clone()),
```

When listing files from a folder, derive folder account from `folder_metadata.account_id` and set `FileMetadata.account_id`.

- [ ] **Step 3: Make delete update accounting**

After successful delete:

```rust
delete_file_accounting(&conn, &resolved_account_id, folder_id, message_id as i64)?;
```

Add helper:

```rust
pub fn delete_file_accounting(conn: &Connection, account_id: &str, folder_id: Option<i64>, message_id: i64) -> Result<(), String> {
    let mut stmt = conn.prepare("DELETE FROM file_accounting WHERE account_id = ? AND folder_id IS ? AND message_id = ?").map_err(|e| e.to_string())?;
    stmt.bind((1, account_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, folder_id)).map_err(|e| e.to_string())?;
    stmt.bind((3, message_id)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    refresh_account_totals(conn, account_id)
}
```

- [ ] **Step 4: Run checks and commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/fs.rs app/src-tauri/src/commands/accounts.rs app/src-tauri/src/commands/folder_groups.rs app/src/types.ts
git commit -m "feat: track storage usage by account"
```

Expected: checks pass and commit succeeds.

## Task 12: Account Sync Commands

**Files:**
- Modify: `app/src-tauri/src/commands/accounts.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/src/hooks/useAccounts.ts`
- Modify: `app/src/components/desktop/dashboard/StorageAccountsPanel.tsx`

- [ ] **Step 1: Add sync command**

Add command:

```rust
#[tauri::command]
pub async fn cmd_sync_account_storage(
    account_id: String,
    db_pool: State<'_, DbConnection>,
) -> Result<AccountStorageSummary, String> {
    let conn = db_pool.lock().map_err(|_| "DB poisoned".to_string())?;
    refresh_account_totals(&conn, &account_id)?;
    let now = chrono::Utc::now().timestamp();
    let mut stmt = conn.prepare("UPDATE telegram_accounts SET last_sync_at = ?, last_error = NULL WHERE account_id = ?").map_err(|e| e.to_string())?;
    stmt.bind((1, now)).map_err(|e| e.to_string())?;
    stmt.bind((2, account_id.as_str())).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    account_summary_from_db(&conn)
}
```

Register in `lib.rs`.

- [ ] **Step 2: Add hook mutation**

In `useAccounts.ts`:

```ts
export function useSyncAccountStorage() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (accountId: string) => invoke<AccountStorageSummary>('cmd_sync_account_storage', { accountId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['telegram-accounts'] });
      queryClient.invalidateQueries({ queryKey: ['account-storage-summary'] });
    },
  });
}
```

- [ ] **Step 3: Add per-account Sync button**

In account card:

```tsx
<button
  type="button"
  onClick={() => syncAccount.mutate(account.account_id)}
  className="text-xs px-2 py-1 rounded bg-telegram-hover text-telegram-subtext hover:text-telegram-text"
>
  Sync
</button>
```

- [ ] **Step 4: Run checks and commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/accounts.rs app/src-tauri/src/lib.rs app/src/hooks/useAccounts.ts app/src/components/desktop/dashboard/StorageAccountsPanel.tsx
git commit -m "feat: add account storage sync action"
```

Expected: checks pass and commit succeeds.

## Task 13: Account-Aware Preview, Download, Rename, Move, Share

**Files:**
- Modify: `app/src-tauri/src/commands/fs.rs`
- Modify: `app/src-tauri/src/commands/preview.rs`
- Modify: `app/src-tauri/src/server.rs`
- Modify: `app/src-tauri/src/share_routes.rs`
- Modify: `app/src-tauri/src/commands/sharing.rs`

- [ ] **Step 1: Add resolver helper**

In `accounts.rs`, add:

```rust
pub fn account_for_folder(conn: &Connection, folder_id: Option<i64>) -> Result<String, String> {
    if let Some(id) = folder_id {
        let mut stmt = conn.prepare("SELECT COALESCE(account_id, 'default') FROM folder_metadata WHERE channel_id = ?").map_err(|e| e.to_string())?;
        stmt.bind((1, id)).map_err(|e| e.to_string())?;
        if let Ok(sqlite::State::Row) = stmt.next() {
            return stmt.read::<String, _>(0).map_err(|e| e.to_string());
        }
    }
    Ok(DEFAULT_ACCOUNT_ID.to_string())
}
```

- [ ] **Step 2: Replace file operations one command at a time**

For each operation:

- `cmd_get_files`
- `cmd_rename_file`
- `cmd_delete_file`
- `cmd_download_file`
- `cmd_move_files`
- preview/thumbnail commands
- share route streaming

Add `db_pool: State<'_, DbConnection>` where needed, resolve account via `account_for_folder`, then use the account-aware client helper from Task 10.

For cross-account moves, return:

```rust
return Err("Moving files between different Telegram accounts is not supported in this version".to_string());
```

- [ ] **Step 3: Run command smoke tests after each operation**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
```

Expected after each operation: cargo exits with code `0`.

- [ ] **Step 4: Manual smoke test**

Run app and verify:

- list files in default account folder,
- preview a file,
- download a file,
- rename a file,
- delete a test file,
- move a file within the same account,
- receive clear error when attempting cross-account move.

- [ ] **Step 5: Commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git add app/src-tauri/src/commands/fs.rs app/src-tauri/src/commands/preview.rs app/src-tauri/src/server.rs app/src-tauri/src/share_routes.rs app/src-tauri/src/commands/sharing.rs app/src-tauri/src/commands/accounts.rs
git commit -m "feat: resolve file operations by account"
```

## Task 14: Settings Accounts Section

**Files:**
- Modify: `app/src/components/desktop/dashboard/SettingsModal.tsx`
- Modify: `app/src/hooks/useAccounts.ts`
- Modify: `app/src-tauri/src/commands/accounts.rs`
- Modify: `app/src-tauri/src/lib.rs`

- [ ] **Step 1: Add enable/disable command**

In `accounts.rs`:

```rust
#[tauri::command]
pub async fn cmd_set_account_enabled(
    account_id: String,
    enabled: bool,
    db_pool: State<'_, DbConnection>,
) -> Result<bool, String> {
    let conn = db_pool.lock().map_err(|_| "DB poisoned".to_string())?;
    let status = if enabled { "active" } else { "disabled" };
    let mut stmt = conn.prepare("UPDATE telegram_accounts SET status = ?, updated_at = strftime('%s','now') WHERE account_id = ?").map_err(|e| e.to_string())?;
    stmt.bind((1, status)).map_err(|e| e.to_string())?;
    stmt.bind((2, account_id.as_str())).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(true)
}
```

Register in `lib.rs`.

- [ ] **Step 2: Add hook mutation**

In `useAccounts.ts`:

```ts
export function useSetAccountEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ accountId, enabled }: { accountId: string; enabled: boolean }) =>
      invoke<boolean>('cmd_set_account_enabled', { accountId, enabled }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['telegram-accounts'] });
      queryClient.invalidateQueries({ queryKey: ['account-storage-summary'] });
    },
  });
}
```

- [ ] **Step 3: Add Accounts section to settings**

In `SettingsModal.tsx`, add a section titled `Accounts` near storage/API settings. Render:

- account name,
- status,
- tracked bytes,
- enable/disable button,
- last sync time,
- note that default account cannot be removed in this version.

- [ ] **Step 4: Run checks and commit**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo check
cd "E:\AUTO KLIK\Teledrive"
git add app/src/components/desktop/dashboard/SettingsModal.tsx app/src/hooks/useAccounts.ts app/src-tauri/src/commands/accounts.rs app/src-tauri/src/lib.rs
git commit -m "feat: add accounts settings controls"
```

Expected: checks pass and commit succeeds.

## Task 15: Final Verification And Packaging

**Files:**
- No source edits unless verification finds a defect.

- [ ] **Step 1: Run Rust tests**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app\src-tauri"
cargo test -- --nocapture
```

Expected: all tests pass.

- [ ] **Step 2: Run frontend build**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run build
```

Expected: TypeScript and Vite exit code `0`.

- [ ] **Step 3: Run Tauri build**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive\app"
npm run tauri build
```

Expected: Tauri release app and installer are produced.

- [ ] **Step 4: Manual smoke test checklist**

Verify:

- existing default account opens without requiring login,
- account panel appears on dashboard,
- account panel can be collapsed and restored,
- `Add Account` opens login wizard in modal,
- storage totals render,
- folder lock selector saves selection,
- unlocked upload queues normally,
- locked-folder unavailable account shows fallback dialog,
- preview/download/delete work for default account,
- no ad strings remain in source/build:

```powershell
cd "E:\AUTO KLIK\Teledrive"
rg -n "effectivecpmnetwork|Sponsored Ad|AdGateway|DesktopAdBanner|AdsterraBanner" app/src app/dist app/src-tauri -S
```

Expected: no matches.

- [ ] **Step 5: Copy master app to root**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
Copy-Item -LiteralPath "app\src-tauri\target\release\app.exe" -Destination "Telegram Drive.exe" -Force
Copy-Item -LiteralPath "app\src-tauri\target\release\bundle\nsis\Telegram Drive_1.9.6_x64-setup.exe" -Destination "Telegram Drive_1.9.6_x64-setup.exe" -Force
```

Expected: root master app is refreshed.

- [ ] **Step 6: Clean build artifacts if user wants small folder**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
Remove-Item -LiteralPath "app\src-tauri\target" -Recurse -Force
Remove-Item -LiteralPath "app\dist" -Recurse -Force
Remove-Item -LiteralPath "app\node_modules" -Recurse -Force
```

Expected: folder size returns near the current compact size. Run `npm install` again before future development.

- [ ] **Step 7: Final commit and push**

Run:

```powershell
cd "E:\AUTO KLIK\Teledrive"
git status -sb
git push origin codex/multi-account-storage
```

Expected: branch pushes to GitHub. Merge to `main` only after manual smoke tests pass.

## Self-Review

Spec coverage:

- One-screen dashboard control is covered by Tasks 6, 7, and 14.
- Account registry and migration are covered by Tasks 1, 2, and 4.
- Hybrid routing and locked-folder fallback are covered by Tasks 3, 7, and 8.
- Account-add flow is covered by Task 9.
- Multi-client backend is covered by Task 10.
- Storage tracking and sync are covered by Tasks 11 and 12.
- Existing operations resolving the right account are covered by Task 13.
- Final verification and compact local folder cleanup are covered by Task 15.

Placeholder scan:

- The plan avoids open-ended implementation markers and gives concrete file paths, command names, and code snippets.

Type consistency:

- Backend uses `account_id`, `locked_account_id`, `TelegramAccount`, `AccountStorageSummary`, and `UploadRouteDecision`.
- Frontend uses matching snake_case fields because Tauri serializes Rust DTOs directly.

