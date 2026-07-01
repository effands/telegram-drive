# Multi-Account Storage Design

## Goal

Upgrade Telegram Drive from a single Telegram session into a multi-account storage manager while keeping the main file manager simple and familiar. Users should control accounts, routing, and combined storage from the existing dashboard screen without turning the app into a complex admin console.

## User Experience

The main dashboard remains the primary screen:

- Left sidebar: folders/groups remain as they are today.
- Center area: file grid/list remains as it is today.
- Top bar: search/view/settings remain as they are today.
- New right-side account/storage panel: shows Telegram accounts, combined usage, sync actions, and routing controls.

The right-side panel should be collapsible so users who want the old simple view can hide it. The default state can be visible after multi-account is enabled, then remembered in settings.

The panel contains:

- Combined storage summary based on local file records.
- Per-account cards with display name, phone/username when available, connection status, tracked bytes, file count, and last sync time.
- `Add Account` quick action that opens the existing login wizard in account-add mode.
- `Sync` action per account to rescan Telegram Drive folders/channels and refresh tracked usage.
- Upload routing mode indicator: `Auto Pool`, `Locked Folder`, or `Manual Fallback Needed`.
- Folder lock control for the active folder, allowing a folder to be pinned to a specific Telegram account.

Settings keeps an Accounts section for advanced management, but routine control happens from the dashboard panel.

## Account Model

Each Telegram account becomes a first-class local entity:

- `account_id`: stable app-generated ID.
- `display_name`: Telegram name when available.
- `phone` or `username`: optional display metadata.
- `session_path`: separate SQLite session file for this account.
- `status`: `active`, `offline`, `rate_limited`, `needs_login`, or `disabled`.
- `is_default`: marks the preferred account for new uploads when no better routing signal exists.
- `tracked_bytes`: total bytes known from local records and latest sync.
- `tracked_files`: total files known from local records and latest sync.
- `last_sync_at`: last successful scan.

Existing single-account installs migrate into one account record and reuse the current `telegram.session` file, then future sessions use per-account paths such as:

```text
sessions/<account_id>/telegram.session
```

The migration must preserve the current login, folders, files, and settings.

## Backend Architecture

Current backend state stores one `Client`. Multi-account requires an account pool:

- `AccountManager`: loads account metadata, opens sessions, and manages lifecycle for each account.
- `TelegramAccountClient`: wraps a Telegram `Client`, runner shutdown handle, API ID, status, and last error.
- `AccountResolver`: finds the correct account for file operations by reading `account_id` from file/folder records.
- `UploadRouter`: chooses an account for new uploads based on folder lock, account status, and local tracked usage.

Commands that currently read `state.client` should move gradually behind helper methods:

- Operations on existing files resolve the file or folder account first.
- Upload operations call the upload router before opening Telegram APIs.
- Folder operations include account ownership.
- Sync commands run per account or all accounts.

The backend should support multiple clients loaded at the same time, but initial implementation can connect accounts lazily when needed to keep memory and network activity modest.

## Data Model

Local database records need account ownership:

- Accounts table stores account metadata and sync status.
- Folder/channel records gain `account_id`.
- File records gain `account_id`.
- Upload queue items gain `account_id` once routed.
- Folder lock table or folder column stores the preferred/locked account for a virtual folder.

For backward compatibility:

- Existing folder/file rows without `account_id` are assigned to the migrated default account.
- Existing store keys `api_id`, `api_hash`, and `folders` remain readable for migration.
- After migration, new code reads account records instead of assuming a single global session.

## Hybrid Upload Routing

Hybrid mode is the default:

1. If active folder is locked to an account, route upload to that account.
2. If locked account is available, upload proceeds.
3. If locked account is unavailable, rate-limited, or over the user-defined soft limit, pause and ask the user before falling back.
4. If active folder is not locked, choose the best account automatically.

The best automatic account is selected by:

- enabled status,
- active session or reconnectable session,
- no current rate-limit cooldown,
- lowest tracked bytes or a balanced strategy,
- optional user preference for default account.

Telegram does not expose a reliable total storage quota. Storage indicators are therefore based on app-managed records and sync scans.

## Sync And Storage Accounting

The dashboard uses local records for fast display:

- total tracked bytes per account,
- total tracked files per account,
- combined totals across enabled accounts.

Sync refreshes records by scanning Telegram Drive folders/channels for a selected account:

- `Sync Account`: refresh one account.
- `Sync All`: refresh every enabled account.

Sync should not block the main UI. It should show progress/status in the account panel and keep existing file browsing usable.

## Error Handling

Important states must be visible and actionable:

- `needs_login`: account session expired; show reconnect action.
- `rate_limited`: show cooldown or last error.
- `offline`: show retry/reconnect.
- `disabled`: ignore account for routing until user enables it.
- locked-folder account unavailable: pause upload and ask whether to fallback, retry, or cancel.

The app should never silently upload a locked folder's files to another account.

## UI Constraints

The dashboard should remain simple:

- Do not redesign the whole app.
- Do not replace the current file manager layout.
- Do not add a separate required workflow for normal upload.
- Keep the account panel compact and collapsible.
- Prefer concise cards and status pills over large explanatory text.

## Testing Strategy

Backend tests should cover:

- migration from single account to default account,
- upload routing for unlocked folders,
- upload routing for locked folders,
- fallback prompt requirement when locked account is unavailable,
- account resolution for preview/download/delete/move.

Frontend tests should cover:

- account panel renders combined totals,
- folder lock selector updates active folder routing,
- add account action opens account-add login flow,
- locked-account failure displays a decision prompt.

Manual smoke tests:

- Existing single account still opens after migration.
- Add second account.
- Upload to unlocked folder and confirm account assignment.
- Lock a folder to account A and upload.
- Simulate account A unavailable and confirm the app asks before fallback.
- Preview/download files from both accounts.

## Out Of Scope For First Implementation

- True Telegram quota detection.
- Automatic account creation.
- Cross-account deduplication.
- Moving existing Telegram messages between different accounts.
- Web hosting version of the desktop-only backend.

