import type { ParseSpan, RepeatRule, Task } from '../../lib/ipc/bindings';
import type { TimeFormat } from '../../lib/time/format';
import type { WordMark } from '../../lib/ui/mark-words';
import { dayText } from '../../lib/ui/when-grid';
import { repeatLabel } from '../../lib/ui/when-repeat';
import { formatHm } from '../../lib/ui/when-time';
import { taskRepeat } from './dates';

/**
 * What the parser understood in a Task's title, as stored in `Task.understood`: the words as
 * typed, the kind of thing they meant and the value they set (a date `YYYY-MM-DD`, a time
 * `HH:MM`, the Repeat as JSON, a Priority number, a List's id). The rows and the sheet underline
 * those words with "Understood as …", until the field is changed by hand.
 */
export type UnderstoodKind = 'date' | 'time' | 'repeat' | 'prio' | 'list';

export interface Understood {
  word: string;
  kind: UnderstoodKind;
  value: string;
}

/** The fields of a Task an understood word can have set. */
export type UnderstoodFields = Pick<
  Task,
  | 'due_date'
  | 'due_time'
  | 'repeat_freq'
  | 'repeat_every'
  | 'repeat_days'
  | 'repeat_mode'
  | 'priority'
  | 'list_id'
>;

const KINDS: readonly string[] = ['date', 'time', 'repeat', 'prio', 'list'];

/** The stored record; anything unreadable is no record. */
export function parseUnderstood(json: string | null): Understood[] {
  if (!json) return [];
  try {
    const list: unknown = JSON.parse(json);
    if (!Array.isArray(list)) return [];
    return list.filter(
      (e): e is Understood =>
        typeof e === 'object' &&
        e !== null &&
        typeof e.word === 'string' &&
        typeof e.value === 'string' &&
        KINDS.includes(e.kind),
    );
  } catch {
    return [];
  }
}

/** The record to store, or none when nothing was understood. */
export const serializeUnderstood = (entries: Understood[]): string | null =>
  entries.length === 0
    ? null
    : JSON.stringify(entries.map(({ word, kind, value }) => ({ word, kind, value })));

/** A Repeat read back from its JSON, or none. */
export function parseRepeat(value: string): RepeatRule | null {
  try {
    const r: unknown = JSON.parse(value);
    if (typeof r !== 'object' || r === null) return null;
    const { freq, every, days, mode } = r as Record<string, unknown>;
    if (typeof freq !== 'string' || typeof every !== 'number' || typeof mode !== 'string') {
      return null;
    }
    return { freq, every, days: typeof days === 'number' ? days : null, mode };
  } catch {
    return null;
  }
}

/** Whether two Repeats are the same rule. Only weekly ones keep their days: for the others the
 * day is just the Due date's own, which moves when the date does. */
function sameRepeat(a: RepeatRule | null, b: RepeatRule | null): boolean {
  if (a === null || b === null) return a === b;
  return (
    a.freq === b.freq &&
    a.every === b.every &&
    a.mode === b.mode &&
    (a.freq !== 'week' || (a.days ?? 0) === (b.days ?? 0))
  );
}

/**
 * The drift rule: a word is still understood while the field it set still has the value it set.
 * Change the date, time, Repeat, Priority or List by hand and its words stop being underlined.
 */
export function stillHolds(u: Understood, fields: UnderstoodFields): boolean {
  switch (u.kind) {
    case 'date':
      return fields.due_date === u.value;
    case 'time':
      return fields.due_time === u.value;
    case 'prio':
      return String(fields.priority) === u.value;
    case 'list':
      return fields.list_id === u.value;
    case 'repeat':
      return sameRepeat(parseRepeat(u.value), taskRepeat(fields as Task));
  }
}

/** The understood words that still hold. */
export const liveUnderstood = (task: Pick<Task, 'understood'> & UnderstoodFields): Understood[] =>
  parseUnderstood(task.understood).filter((u) => stillHolds(u, task));

/** What the meaning of a word needs to be shown in the user's terms. */
export interface MeaningContext {
  today: string;
  timeFormat: TimeFormat;
  /** A List's name from its id, or none when it is gone. */
  listName: (id: string) => string | null;
}

/** A value as words: `Fri 23 Oct`, `9:30 PM`, `Every 2 weeks`, `P1`, a List's name. */
export function meaningText(kind: UnderstoodKind, value: string, cx: MeaningContext): string {
  switch (kind) {
    case 'date':
      return dayText(value, cx.today);
    case 'time':
      return formatHm(value, cx.timeFormat);
    case 'prio':
      return `P${value}`;
    case 'repeat': {
      const rule = parseRepeat(value);
      return rule ? repeatLabel(rule) : value;
    }
    case 'list':
      return cx.listName(value) ?? 'a List that is gone';
  }
}

/** A parsed word as the chip says it: a List by its name, a new one as an offer. */
export function spanMeaning(span: ParseSpan, cx: Pick<MeaningContext, 'today' | 'timeFormat'>) {
  if (span.kind === 'list') return span.new_list ? `New list “${span.value}”` : span.value;
  return meaningText(span.kind as UnderstoodKind, span.value, { ...cx, listName: () => null });
}

/**
 * The marks for a Task's title: each word still understood, with "Understood as …" as its
 * tooltip. Words that share their text ("tonight" sets a date and a time) share one mark.
 */
export function understoodMarks(
  task: Pick<Task, 'understood'> & UnderstoodFields,
  cx: MeaningContext,
): WordMark[] {
  const byWord = new Map<string, string[]>();
  for (const u of liveUnderstood(task)) {
    const key = u.word.toLowerCase();
    byWord.set(key, [...(byWord.get(key) ?? []), meaningText(u.kind, u.value, cx)]);
  }
  return [...byWord].map(([word, meanings]) => ({
    word,
    title: `Understood as ${meanings.join(' · ')}`,
  }));
}

/**
 * The record for a captured Task: the words that won, with a List's id in place of its name
 * (`listId` finds it, for a List that was just created too).
 */
export function understoodFromSpans(
  spans: ParseSpan[],
  listId: (span: ParseSpan) => string | null,
): Understood[] {
  const out: Understood[] = [];
  for (const s of spans) {
    const value = s.kind === 'list' ? listId(s) : s.value;
    if (value !== null) out.push({ word: s.word, kind: s.kind as UnderstoodKind, value });
  }
  return out;
}

/**
 * The record after a Quick edit: the old words that are still in the title and still hold on the
 * edited Task, plus what the edit's new words meant. A kind the new words set replaces the old
 * words of that kind, because the field they described has changed.
 */
export function mergeUnderstood(
  old: Understood[],
  fresh: Understood[],
  title: string,
  fields: UnderstoodFields,
): Understood[] {
  const low = title.toLowerCase();
  const replaced = new Set(fresh.map((u) => u.kind));
  const kept = old.filter(
    (u) => !replaced.has(u.kind) && low.includes(u.word.toLowerCase()) && stillHolds(u, fields),
  );
  return [...kept, ...fresh];
}
