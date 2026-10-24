import type { Entry, JournalEntryDeletedEvent } from '../../lib/ipc/bindings';
import { dayLabel } from './dates';

/** "‘Wed 21 Oct’ was deleted on Pixel 8": the toast when the Entry in the editor is deleted elsewhere (an Entry has no title, its Page names it). */
export function remoteDeletedMessage(e: JournalEntryDeletedEvent, today: string): string {
  return `‘${dayLabel(e.page_date, today)}’ was deleted on ${e.device ?? 'another device'}`;
}

/** What to do when an Entry is deleted on another device: only the Entry open in the editor is told. */
export function followEntryDelete(
  openId: string | null,
  e: JournalEntryDeletedEvent,
  today: string,
): { message: string } | null {
  return openId !== null && openId === e.entry_id
    ? { message: remoteDeletedMessage(e, today) }
    : null;
}

export interface RestoreDeps {
  restore: (id: string) => Promise<Pick<Entry, 'body'>>;
  update: (id: string, text: string, sessionId: string) => Promise<unknown>;
  session: () => string;
}

/**
 * Brings a deleted Entry back with what the person had typed: the text the editor held that was
 * not yet saved when the delete arrived is saved on top of the restored Entry (blank text never
 * is, as everywhere in the Journal).
 */
export async function restoreWithDraft(
  deps: RestoreDeps,
  id: string,
  draft: string,
): Promise<void> {
  const restored = await deps.restore(id);
  if (draft.trim() !== '' && draft !== restored.body) {
    await deps.update(id, draft, deps.session());
  }
}
