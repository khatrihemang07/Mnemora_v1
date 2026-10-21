/**
 * The browser mock's Task History: the same readable lines Rust builds from change-log snapshots
 * (crates/todo/src/history.rs). The shared rule fixtures keep the two in step (ADR 0009).
 */

import { repeatLabel } from '../ui/when-repeat';
import type { HistoryLine } from './bindings';

/** One change-log record of a Task, as the mock keeps it. */
export interface LogRecord {
  op: 'create' | 'update' | 'move' | 'tick' | 'delete' | 'restore';
  /** The editing session; a session keeps one record. */
  session: string | null;
  /** The Task as it was after the change, plus `deleted_at`. */
  snapshot: Record<string, unknown>;
  at: number;
  /** Insertion order, to break ties in `at` like the rowid does in Rust. */
  seq: number;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

/** `Thu 22 Oct` for a `YYYY-MM-DD` date. */
export function dayText(date: string): string {
  const [y = 0, m = 1, d = 1] = date.split('-').map(Number);
  const weekday = new Date(Date.UTC(y, m - 1, d)).getUTCDay();
  return `${WEEKDAYS[weekday]} ${d} ${MONTHS[m - 1]}`;
}

/** `22 Oct`: the day without its weekday. */
const shortDay = (date: string): string => dayText(date).split(' ').slice(1).join(' ');

const epochDay = (date: string): number => {
  const [y = 0, m = 1, d = 1] = date.split('-').map(Number);
  return Date.UTC(y, m - 1, d) / 86_400_000;
};

/** `Thu` and `Sat` within a week of each other, else whole days (a bare weekday would be ambiguous). */
function skipDays(from: string, to: string): [string, string] {
  const near = Math.abs(epochDay(to) - epochDay(from)) < 7;
  const name = (d: string) => (near ? dayText(d).split(' ')[0] : dayText(d)) as string;
  return [name(from), name(to)];
}

const str = (s: Record<string, unknown>, k: string): string | null =>
  typeof s[k] === 'string' ? (s[k] as string) : null;
const num = (s: Record<string, unknown>, k: string): number | null =>
  typeof s[k] === 'number' ? (s[k] as number) : null;

function repeatOf(s: Record<string, unknown>): string | null {
  const freq = str(s, 'repeat_freq');
  const every = num(s, 'repeat_every');
  if (freq === null || every === null) return null;
  return repeatLabel({
    freq,
    every,
    days: num(s, 'repeat_days'),
    mode: str(s, 'repeat_mode') ?? 'fixed',
  });
}

type Line = [HistoryLine['kind'], string];

/** The lines one record adds to its predecessor, in reading order. */
function recordLines(
  prev: Record<string, unknown> | null,
  base: { title: string | null; notes: string | null },
  rec: LogRecord,
  listLabel: (id: string | null) => string,
): Line[] {
  const cur = rec.snapshot;
  if (prev === null) {
    if (rec.op !== 'create') return [];
    if (str(cur, 'repeat_of') === null) return [['create', 'Created']];
    const due = str(cur, 'due_date');
    return [['done', due ? `Done (occurrence of ${shortDay(due)})` : 'Done']];
  }
  const out: Line[] = [];
  const wasDeleted = prev.deleted_at != null;
  const isDeleted = cur.deleted_at != null;
  if (!wasDeleted && isDeleted) out.push(['delete', 'Deleted']);
  if (wasDeleted && !isDeleted) out.push(['restore', 'Restored']);

  const oldDue = str(prev, 'due_date');
  const newDue = str(cur, 'due_date');
  const step =
    rec.op === 'tick' &&
    str(prev, 'state') === str(cur, 'state') &&
    oldDue !== null &&
    newDue !== null &&
    oldDue !== newDue;
  if (step && oldDue !== null && newDue !== null) {
    const [from, to] = skipDays(oldDue, newDue);
    out.push(
      cur.skipped === true
        ? ['skip', `Skipped (${from} → ${to})`]
        : newDue > oldDue
          ? ['done', `Done (occurrence of ${shortDay(oldDue)})`]
          : ['reopen', `Reopened (occurrence of ${shortDay(newDue)})`],
    );
  } else if (str(prev, 'state') !== str(cur, 'state')) {
    const state = str(cur, 'state');
    out.push(
      state === 'done'
        ? ['done', 'Done']
        : state === 'wont'
          ? ['wont', 'Won’t do']
          : ['reopen', 'Reopened'],
    );
  }

  if (str(prev, 'list_id') !== str(cur, 'list_id')) {
    out.push([
      'list',
      `Moved ${listLabel(str(prev, 'list_id'))} → ${listLabel(str(cur, 'list_id'))}`,
    ]);
  }
  if (!step && oldDue !== newDue) {
    out.push([
      'date',
      `Due date ${oldDue === null ? 'no date' : dayText(oldDue)} → ${newDue === null ? 'no date' : dayText(newDue)}`,
    ]);
  }
  const oldTime = str(prev, 'due_time');
  const newTime = str(cur, 'due_time');
  if (oldTime !== newTime) {
    if (oldTime === null) out.push(['time', `Time set to ${newTime}`]);
    else if (newTime === null) out.push(['time', 'Time removed']);
    else out.push(['time', `Time ${oldTime} → ${newTime}`]);
  }
  if (!step) {
    const oldRepeat = repeatOf(prev);
    const newRepeat = repeatOf(cur);
    if (oldRepeat !== newRepeat) {
      if (oldRepeat === null) out.push(['repeat', `Repeat set: ${newRepeat}`]);
      else if (newRepeat === null) out.push(['repeat', 'Repeat removed']);
      else out.push(['repeat', `Repeat ${oldRepeat} → ${newRepeat}`]);
    }
  }
  if (num(prev, 'priority') !== num(cur, 'priority')) {
    out.push([
      'priority',
      `Priority P${num(prev, 'priority') ?? 4} → P${num(cur, 'priority') ?? 4}`,
    ]);
  }
  // Only an editing session edits text, and its record can be rewritten after other changes, so
  // the text is compared with what it was before the session (`base`), not with the neighbour.
  if (rec.session !== null) {
    if (str(cur, 'title') !== base.title) out.push(['title', 'Title edited']);
    if (str(cur, 'notes') !== base.notes) out.push(['notes', 'Notes edited']);
  }
  return out;
}

/** A Task's History from its records, newest line first. */
export function historyOf(
  records: LogRecord[],
  listLabel: (id: string | null) => string,
): HistoryLine[] {
  const ordered = [...records].sort((a, b) => a.at - b.at || a.seq - b.seq);
  const batches: HistoryLine[][] = [];
  let prev: Record<string, unknown> | null = null;
  let base: { title: string | null; notes: string | null } | null = null;
  for (const rec of ordered) {
    const text = { title: str(rec.snapshot, 'title'), notes: str(rec.snapshot, 'notes') };
    batches.push(
      recordLines(prev, base ?? text, rec, listLabel).map(([kind, line]) => ({
        at: rec.at,
        kind,
        text: line,
      })),
    );
    // A session settles the text; so do creating and a tick (a Checklist reset).
    if (base === null || rec.session !== null || rec.op === 'create' || rec.op === 'tick') {
      base = text;
    }
    prev = rec.snapshot;
  }
  return batches.reverse().flat();
}
