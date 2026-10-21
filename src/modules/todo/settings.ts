import type { DoneInLists, JsonValue } from '../../lib/ipc/bindings';

/** The Preference behind "Recognise dates in titles". */
export const PARSE_DATES_PREF = 'todo.parse_dates';

const DONE_MODES: readonly DoneInLists[] = ['today_shown', 'folded', 'shown', 'hidden'];

/** Typed view of the `todo.*` Preferences the Settings section edits. Rust validates. */
export interface TodoSettings {
  /** Whether the capture box reads dates, times, Repeats, #List and p1-p4 out of typed text. */
  parseDates: boolean;
  doneInLists: DoneInLists;
}

export function readTodo(values: Record<string, JsonValue> = {}): TodoSettings {
  const mode = values['todo.done_in_lists'];
  return {
    parseDates: values[PARSE_DATES_PREF] !== false,
    doneInLists: DONE_MODES.includes(mode as DoneInLists) ? (mode as DoneInLists) : 'today_shown',
  };
}
