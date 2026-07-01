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
    #[serde(skip_serializing)]
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

    #[test]
    fn telegram_account_does_not_serialize_session_path() {
        let account = TelegramAccount {
            account_id: "acct_a".to_string(),
            display_name: "Primary".to_string(),
            phone: Some("+12025550100".to_string()),
            username: Some("primary_user".to_string()),
            session_path: "C:\\Users\\test\\secret.session".to_string(),
            status: TelegramAccountStatus::Active,
            is_default: true,
            tracked_bytes: 1024,
            tracked_files: 2,
            last_sync_at: Some(1_725_000_000),
            last_error: None,
        };

        let value = serde_json::to_value(account).unwrap();

        assert_eq!(value["account_id"], "acct_a");
        assert!(value.get("session_path").is_none());
    }

    #[test]
    fn file_metadata_deserializes_legacy_payload_without_account_id() {
        let json = r#"{
            "id": 1,
            "folder_id": null,
            "name": "report.pdf",
            "size": 4096,
            "mime_type": "application/pdf",
            "file_ext": "pdf",
            "created_at": "2026-07-01T00:00:00Z",
            "icon_type": "pdf"
        }"#;

        let metadata: FileMetadata = serde_json::from_str(json).unwrap();

        assert_eq!(metadata.account_id, None);
        assert_eq!(metadata.name, "report.pdf");
    }

    #[test]
    fn folder_metadata_deserializes_legacy_payload_without_account_id() {
        let json = r#"{
            "id": 7,
            "parent_id": null,
            "name": "Archive",
            "username": null,
            "is_public": false,
            "group_id": null,
            "display_order": 3
        }"#;

        let metadata: FolderMetadata = serde_json::from_str(json).unwrap();

        assert_eq!(metadata.account_id, None);
        assert_eq!(metadata.name, "Archive");
    }
}
