use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "status", content = "data")]
pub enum AuthState {
    LoggedOut,
    AwaitingCode { phone: String, phone_code_hash: String },
    AwaitingPassword { phone: String },
    LoggedIn,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthResult {
    pub success: bool,
    pub next_step: Option<String>, // "code", "password", "dashboard"
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileMetadata {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub account_id: Option<String>,
    pub name: String,
    pub size: u64, // Updated to u64
    pub mime_type: Option<String>,
    pub file_ext: Option<String>, // Added field
    pub created_at: String, 
    pub icon_type: String, 
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FolderMetadata {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub account_id: Option<String>,
    pub name: String,
    /// Telegram public username (e.g. "mychannel"). None if private.
    pub username: Option<String>,
    /// Whether the channel is public (has a username set).
    pub is_public: bool,
    // Local-first grouping & ordering metadata
    pub group_id: Option<i32>,
    pub display_order: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FolderGroup {
    pub id: i32,
    pub name: String,
    pub color_hex: String,
    pub display_order: i32,
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Drive {
    pub chat_id: i64,
    pub name: String,
    pub icon: Option<String>,
}

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
