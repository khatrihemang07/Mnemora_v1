import { toast } from 'svelte-sonner';

/** Plain confirmation. */
export const notify = (message: string) => toast(message);

export const notifyError = (message: string) => toast.error(message);

/** How long the Undo stays on offer. */
const UNDO_MS = 5000;

/** A confirmation with one follow-up action, e.g. "Moved to Mon 19 Oct · Show". */
export const notifyAction = (message: string, label: string, onClick: () => void) =>
  toast(message, { action: { label, onClick } });

/** N10-A: destructive actions complete at once and offer Undo for the toast's lifetime. */
export const notifyUndo = (message: string, onUndo: () => void) =>
  toast(message, { duration: UNDO_MS, action: { label: 'Undo', onClick: onUndo } });
