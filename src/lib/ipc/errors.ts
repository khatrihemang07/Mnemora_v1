import { commands } from './bindings';

const MAX_KEPT = 5;
const recent: { at: string; message: string }[] = [];

/** Last few UI errors, for "Copy diagnostics". */
export const recentUiErrors = () => [...recent];

function report(message: string, stack: string | null) {
  recent.push({ at: new Date().toISOString(), message });
  if (recent.length > MAX_KEPT) recent.shift();
  // Never let reporting itself raise another unhandled error.
  void commands.logUiError(message, stack).catch(() => {});
}

export function installGlobalErrorHandlers() {
  window.addEventListener('error', (e) => {
    report(e.message, e.error instanceof Error ? (e.error.stack ?? null) : null);
  });
  window.addEventListener('unhandledrejection', (e) => {
    const r = e.reason;
    report(
      r instanceof Error ? r.message : String(r),
      r instanceof Error ? (r.stack ?? null) : null,
    );
  });
}
