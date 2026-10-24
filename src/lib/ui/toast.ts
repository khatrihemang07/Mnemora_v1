import { toast } from 'svelte-sonner';

/** Lifetimes in ms: a toast with a button, and every error, stays for Important. */
export const IMPORTANT_MS = 5000;
/** A plain confirmation or notice, and a copy or open-folder failure, stays for Minor. */
export const MINOR_MS = 2000;

/** Plain confirmation. */
export const notify = (message: string, duration = MINOR_MS) => toast(message, { duration });

/** A failure: Important, unless a Minor `duration` is passed (copy and open-folder failures). */
export const notifyError = (message: string, duration = IMPORTANT_MS) =>
  toast.error(message, { duration });

/** A confirmation with one follow-up action, e.g. "Moved to Mon 19 Oct · Show". */
export const notifyAction = (message: string, label: string, onClick: () => void) =>
  toast(message, { duration: IMPORTANT_MS, action: { label, onClick } });

/**
 * N10-A: destructive actions complete at once and offer Undo for the toast's lifetime. A toast
 * with the same `id` replaces the one on show (a picker that changes the same thing again and again).
 */
export const notifyUndo = (message: string, onUndo: () => void, id?: string) =>
  toast(message, { id, duration: IMPORTANT_MS, action: { label: 'Undo', onClick: onUndo } });
