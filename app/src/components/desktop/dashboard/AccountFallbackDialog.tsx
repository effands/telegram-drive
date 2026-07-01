import type { UploadRouteDecision } from '../../../types';

interface AccountFallbackDialogProps {
    decision: UploadRouteDecision;
    count: number;
    onRetry: () => void;
    onUseFallback: () => void;
    onCancel: () => void;
}

export function AccountFallbackDialog({
    decision,
    count,
    onRetry,
    onUseFallback,
    onCancel,
}: AccountFallbackDialogProps) {
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
                    <button onClick={onCancel} className="px-3 py-2 rounded bg-telegram-hover text-telegram-text">
                        Cancel
                    </button>
                    <button onClick={onRetry} className="px-3 py-2 rounded bg-telegram-hover text-telegram-text">
                        Retry
                    </button>
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
