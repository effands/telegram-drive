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
