import { Lock, Unlock } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import type { TelegramAccount, TelegramFolder } from '../../../types';

interface FolderAccountLockProps {
    folder: TelegramFolder | null;
    accounts: TelegramAccount[];
    onChanged: () => void | Promise<void>;
}

export function FolderAccountLock({ folder, accounts, onChanged }: FolderAccountLockProps) {
    if (!folder) return null;

    const lockedAccountId = folder.locked_account_id ?? '';

    return (
        <div className="rounded-lg border border-telegram-border bg-telegram-bg/60 p-3">
            <div className="flex items-center gap-2 mb-2">
                {lockedAccountId ? (
                    <Lock className="w-4 h-4 text-amber-400" />
                ) : (
                    <Unlock className="w-4 h-4 text-telegram-subtext" />
                )}
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
                    await onChanged();
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
