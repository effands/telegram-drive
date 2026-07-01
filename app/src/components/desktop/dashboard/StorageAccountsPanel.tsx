import { AlertTriangle, Database, PanelRightClose, Plus, RefreshCw, ShieldCheck } from 'lucide-react';
import { useAccountStorageSummary, useSyncAccountStorage } from '../../../hooks/useAccounts';
import type { TelegramFolder } from '../../../types';
import { formatBytes } from '../../../utils';
import { FolderAccountLock } from './FolderAccountLock';

interface StorageAccountsPanelProps {
    activeFolder: TelegramFolder | null;
    onAddAccount: () => void;
    onFolderLockChanged: () => void | Promise<void>;
    onClose: () => void;
}

function statusTone(status: string) {
    if (status === 'active') return 'text-emerald-400 bg-emerald-500/10';
    if (status === 'needs_login' || status === 'rate_limited') return 'text-amber-400 bg-amber-500/10';
    return 'text-rose-400 bg-rose-500/10';
}

export function StorageAccountsPanel({
    activeFolder,
    onAddAccount,
    onFolderLockChanged,
    onClose,
}: StorageAccountsPanelProps) {
    const { data, isLoading, refetch } = useAccountStorageSummary();
    const syncAccount = useSyncAccountStorage();
    const accounts = data?.accounts ?? [];

    return (
        <aside className="hidden xl:flex w-80 shrink-0 border-l border-telegram-border bg-telegram-surface/50 flex-col">
            <div className="p-4 border-b border-telegram-border flex items-center justify-between gap-3">
                <div className="min-w-0">
                    <p className="text-xs uppercase tracking-wide text-telegram-subtext font-semibold">Storage Accounts</p>
                    <h2 className="text-lg font-semibold text-telegram-text">{formatBytes(data?.total_bytes ?? 0)}</h2>
                </div>
                <div className="flex items-center gap-1">
                    <button
                        type="button"
                        onClick={() => refetch()}
                        className="p-2 rounded-lg hover:bg-telegram-hover text-telegram-subtext hover:text-telegram-text"
                        title="Refresh account summary"
                    >
                        <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
                    </button>
                    <button
                        type="button"
                        onClick={onClose}
                        className="p-2 rounded-lg hover:bg-telegram-hover text-telegram-subtext hover:text-telegram-text"
                        title="Hide account panel"
                    >
                        <PanelRightClose className="w-4 h-4" />
                    </button>
                </div>
            </div>

            <div className="p-3 space-y-2 overflow-y-auto">
                <FolderAccountLock
                    folder={activeFolder}
                    accounts={accounts}
                    onChanged={async () => {
                        await refetch();
                        await onFolderLockChanged();
                    }}
                />

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
                        <div className="mt-3 flex justify-end">
                            <button
                                type="button"
                                onClick={() => syncAccount.mutate(account.account_id)}
                                disabled={syncAccount.isPending}
                                className="text-xs px-2 py-1 rounded bg-telegram-hover text-telegram-subtext hover:text-telegram-text disabled:opacity-50"
                            >
                                Sync
                            </button>
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
                                <span>{account.last_error}</span>
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
