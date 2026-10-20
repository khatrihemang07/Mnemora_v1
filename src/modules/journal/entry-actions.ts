import type { Entry } from '../../lib/ipc/bindings';
import { toggleTaskLine } from '../../lib/ui/markdown/tasks';
import { notify, notifyError, notifyUndo } from '../../lib/ui/toast';
import type { Editing } from './editing.svelte';
import {
  deleteEntry,
  invalidateJournal,
  patchEntry,
  removeCachedEntry,
  restoreEntry,
  toggleTask,
} from './queries';

/** Ticks or unticks the item on `line`. Shows the new state at once; a failure puts it back. */
export async function tickTask(entry: Entry, line: number): Promise<void> {
  const body = toggleTaskLine(entry.body, line);
  if (body === null) return;
  patchEntry({ ...entry, body });
  try {
    await toggleTask(entry.id, line);
  } catch {
    void invalidateJournal();
    notifyError("Couldn't update the checklist");
  }
}

/** Copies the Entry's Markdown. */
export async function copyEntryText(entry: Entry): Promise<void> {
  try {
    await navigator.clipboard.writeText(entry.body);
    notify('Copied');
  } catch {
    notifyError("Couldn't copy");
  }
}

/**
 * Deletes at once and offers Undo for a few seconds. An Entry open for editing is finished first,
 * so its last keystrokes are saved before it goes (and the restore brings them back).
 */
export async function deleteEntryWithUndo(entry: Entry, editing: Editing): Promise<void> {
  if (editing.id === entry.id) {
    await editing.finisher?.();
    editing.close();
  }
  removeCachedEntry(entry.id);
  try {
    await deleteEntry(entry.id);
  } catch {
    void invalidateJournal();
    notifyError("Couldn't delete the Entry");
    return;
  }
  notifyUndo('Entry deleted', () => {
    restoreEntry(entry.id)
      .catch(() => notifyError("Couldn't restore the Entry"))
      .finally(() => void invalidateJournal());
  });
}
