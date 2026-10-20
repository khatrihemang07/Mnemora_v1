import type { Page } from '../../lib/ipc/bindings';

/** An Entry in feed order, with the Page it sits on. */
export interface FeedEntryRef {
  id: string;
  date: string;
}

/** Every Entry of the loaded Pages, top to bottom as the feed shows them (newest Page first). */
export function feedEntries(pages: Page[]): FeedEntryRef[] {
  return pages.flatMap((p) => p.entries.map((e) => ({ id: e.id, date: p.date })));
}

/**
 * The Entry ↑ from an empty composer lands on: the last Entry of Today's Page, or, when Today has
 * none, the last Entry of the newest Page before it. Future Pages are never the answer.
 */
export function newestEntryId(entries: FeedEntryRef[], today: string): string | null {
  const date = entries.find((e) => e.date <= today)?.date;
  return [...entries].reverse().find((e) => e.date === date)?.id ?? null;
}

/** Which Entry the keyboard is on. `wantsMore` says ↓ ran into the end while older Pages remain. */
export interface FocusState {
  id: string | null;
  wantsMore: boolean;
}

export const NO_FOCUS: FocusState = { id: null, wantsMore: false };

export type FocusEvent =
  | { type: 'up' }
  | { type: 'down' }
  /** Focus landed on an Entry (Tab, a click). */
  | { type: 'focus'; id: string }
  | { type: 'clear' }
  /** ↑ in an empty composer. */
  | { type: 'from-composer'; today: string }
  /** Enter, or a click, opened the Entry's editor: it keeps the focus. */
  | { type: 'edit'; id: string }
  /** Esc closed the editor: the Entry stays focused, ready for the next key. */
  | { type: 'stop-edit' }
  /** The focused Entry was deleted (`entries` is the list from before): focus moves on. */
  | { type: 'deleted'; id: string };

/** What the focus does for each event, over the Entries currently loaded. Pure. */
export function focusReducer(
  state: FocusState,
  event: FocusEvent,
  entries: FeedEntryRef[],
): FocusState {
  const at = state.id === null ? -1 : entries.findIndex((e) => e.id === state.id);
  switch (event.type) {
    case 'up': {
      const prev = entries[at - 1];
      return at > 0 && prev ? { id: prev.id, wantsMore: false } : { ...state, wantsMore: false };
    }
    case 'down': {
      if (at < 0) return state;
      const next = entries[at + 1];
      return next ? { id: next.id, wantsMore: false } : { ...state, wantsMore: true };
    }
    case 'focus':
    case 'edit':
      return { id: event.id, wantsMore: false };
    case 'clear':
      return NO_FOCUS;
    case 'from-composer': {
      const id = newestEntryId(entries, event.today);
      return id === null ? state : { id, wantsMore: false };
    }
    case 'stop-edit':
      return state;
    case 'deleted': {
      if (state.id !== event.id) return state;
      const i = entries.findIndex((e) => e.id === event.id);
      const id = entries[i + 1]?.id ?? entries[i - 1]?.id ?? null;
      return { id, wantsMore: false };
    }
  }
}
