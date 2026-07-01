use crate::models::{
    TelegramAccount, TelegramAccountStatus, UploadRouteDecision, UploadRouteStatus,
};

pub fn choose_upload_account(
    accounts: &[TelegramAccount],
    locked_account_id: Option<&str>,
) -> UploadRouteDecision {
    let active_accounts: Vec<&TelegramAccount> = accounts
        .iter()
        .filter(|account| account.status == TelegramAccountStatus::Active)
        .collect();

    if let Some(locked_id) = locked_account_id {
        if let Some(locked) = accounts.iter().find(|account| account.account_id == locked_id) {
            if locked.status == TelegramAccountStatus::Active {
                return UploadRouteDecision {
                    status: UploadRouteStatus::Ready,
                    account_id: Some(locked.account_id.clone()),
                    reason: None,
                    fallback_account_id: None,
                };
            }

            return UploadRouteDecision {
                status: UploadRouteStatus::NeedsUserDecision,
                account_id: None,
                reason: Some(format!("Locked account '{}' is {:?}", locked_id, locked.status)),
                fallback_account_id: active_accounts
                    .iter()
                    .min_by_key(|account| account.tracked_bytes)
                    .map(|account| account.account_id.clone()),
            };
        }

        return UploadRouteDecision {
            status: UploadRouteStatus::NeedsUserDecision,
            account_id: None,
            reason: Some(format!("Locked account '{}' was not found", locked_id)),
            fallback_account_id: active_accounts
                .iter()
                .min_by_key(|account| account.tracked_bytes)
                .map(|account| account.account_id.clone()),
        };
    }

    match active_accounts.iter().min_by_key(|account| account.tracked_bytes) {
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
