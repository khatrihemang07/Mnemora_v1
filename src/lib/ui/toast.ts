import { toast } from 'svelte-sonner';

/** Plain confirmation. */
export const notify = (message: string) => toast(message);

export const notifyError = (message: string) => toast.error(message);

/** N10-A: destructive actions complete at once and offer Undo for the toast's lifetime. */
export const notifyUndo = (message: string, onUndo: () => void) =>
  toast(message, { action: { label: 'Undo', onClick: onUndo } });
