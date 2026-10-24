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
 * is, as everywhere in the Journal). When the restored Entry is not the text the editor started
 * from (it was edited elsewhere too), nothing is overwritten: what was typed after the saved text
 * is added to the end, or the whole draft after a blank line when it is not an extension.
 */
export async function restoreWithDraft(
  deps: RestoreDeps,
  id: string,
  draft: { text: string; saved: string },
): Promise<void> {
  const restored = await deps.restore(id);
  const { text, saved } = draft;
  if (text.trim() === '' || text === restored.body || text === saved) return;
  let next = text;
  if (restored.body !== saved) {
    next = text.startsWith(saved)
      ? restored.body + text.slice(saved.length)
      : `${restored.body}\n\n${text}`;
  }
  if (next !== restored.body) await deps.update(id, next, deps.session());
}
