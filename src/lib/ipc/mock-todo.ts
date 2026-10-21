/**
 * In-memory Todo for the browser-only IPC mock. Mirrors the rules in crates/todo (titles are
 * non-blank, one line and at most 500 characters; new Tasks go last; complete and reopen; Lists
 * with folded-unique names, delete choices with restore, and moving Tasks; Due dates with the
 * past-date and time-only rules, Today and Upcoming groups, Move all overdue; Repeats that record
 * done copies, rewind, skip and the paged Done view). Every
 * result is a JSON copy, like real IPC: handing out live objects would let the UI mutate the
 * "database". The shared rule fixtures (fixtures/todo-rules.json) keep it in step with Rust.
 */

import { addDays } from '../ui/calendar';
import {
  anchoredOn,
  firstMatch,
  nextDue,
  type RepeatRule,
  ruleProblem,
  settleRule,
} from '../ui/when-repeat';
import type {
  AllView,
  AppError,
  Comment,
  Completion,
  DayCount,
  DeleteChoice,
  DoneView,
  DueChange,
  InboxView,
  ListDeletion,
  ListView,
  LocalNow,
  NewWhen,
  OverdueMove,
  SidebarView,
  Task,
  TodayView,
  TodoList,
  UpcomingView,
} from './bindings';
import { historyOf, type LogRecord } from './mock-todo-history';

const MAX_TITLE_CHARS = 500;
const MAX_LIST_NAME_CHARS = 60;
const MAX_EMOJI_CHARS = 16;
const MAX_COMMENT_CHARS = 5000;

const fail = (code: AppError['code'], message: string): AppError => ({
  code,
  message,
  details: null,
});
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v));

/** Base-62 digits in byte order, so keys built from them sort the same as text and as numbers. */
const DIGITS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
const KEY_WIDTH = 8;

/** Mirrors `key_after` in crates/todo/src/task.rs: the key for a Task that goes after all others. */
export function keyAfter(last: string | null): string {
  if (last === null) return `${DIGITS[31]}${'0'.repeat(KEY_WIDTH - 1)}`;
  const counter = last.length === KEY_WIDTH && [...last].every((c) => DIGITS.includes(c));
  if (counter) {
    const out = [...last];
    for (let i = out.length - 1; i >= 0; i--) {
      const at = DIGITS.indexOf(out[i] as string);
      if (at + 1 < DIGITS.length) {
        out[i] = DIGITS[at + 1] as string;
        return out.join('');
      }
      out[i] = DIGITS[0] as string;
    }
  }
  return `${last}${DIGITS[31]}`;
}

/** Mirrors `validate_title`: returns the trimmed title or throws a validation error. */
export function validateTitle(title: string): string {
  const t = title.trim();
  if (!t) throw fail('validation', 'A Task needs a title');
  if (/[\n\r]/.test(t)) throw fail('validation', 'A title must be a single line');
  if ([...t].length > MAX_TITLE_CHARS) {
    throw fail('validation', 'A title can be at most 500 characters');
  }
  return t;
}

/** Mirrors `validate_body`: the trimmed Comment or a validation error. */
export function validateBody(body: string): string {
  const b = body.trim();
  if (!b) throw fail('validation', "A comment can't be blank");
  if ([...b].length > MAX_COMMENT_CHARS) {
    throw fail('validation', 'A comment can be at most 5000 characters');
  }
  return b;
}

/** Mirrors `validate_list_name`: returns the trimmed name or throws a validation error. */
export function validateListName(name: string): string {
  const n = name.trim();
  if (!n) throw fail('validation', 'A List needs a name');
  if (/[\n\r]/.test(n)) throw fail('validation', 'A List name must be a single line');
  if ([...n].length > MAX_LIST_NAME_CHARS) {
    throw fail('validation', 'A List name can be at most 60 characters');
  }
  return n;
}

/** Letters that do not decompose into a base letter plus a mark; mirrors `fold_char` in core. */
const FOLD_EXTRA: Record<string, string> = { đ: 'd', ø: 'o', ł: 'l', ı: 'i' };

/** Mirrors `mnemora_core::text::fold`: lowercase with accents removed. */
export function foldName(name: string): string {
  return [...name.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()]
    .map((c) => FOLD_EXTRA[c] ?? c)
    .join('');
}

/** Mirrors `validate_emoji`: a few characters with no letters or spaces; blank means none. */
export function validateEmoji(emoji: string | null): string | null {
  const e = emoji?.trim();
  if (!e) return null;
  if ([...e].length > MAX_EMOJI_CHARS || /[\s\p{Cc}A-Za-z]/u.test(e)) {
    throw fail('validation', "That doesn't look like an emoji");
  }
  return e;
}

const DAY_MS = 86_400_000;

/** Mirrors `epoch_day`: days since 1970-01-01 of a real `YYYY-MM-DD` date, else null. */
export function epochDay(date: string): number | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return null;
  const [y = 0, m = 0, d = 0] = date.split('-').map(Number);
  const at = new Date(Date.UTC(y, m - 1, d));
  at.setUTCFullYear(y); // years below 100 would otherwise mean 19xx
  const real = at.getUTCFullYear() === y && at.getUTCMonth() === m - 1 && at.getUTCDate() === d;
  return real && y >= 1 ? Math.round(at.getTime() / DAY_MS) : null;
}

/** Mirrors `is_time`: `HH:MM` on a 24-hour clock. */
export const isTime = (time: string): boolean => /^([01]\d|2[0-3]):[0-5]\d$/.test(time);

const checkClock = (local: LocalNow) => {
  if (epochDay(local.today) === null) throw fail('validation', 'Not a valid date');
  if (!isTime(local.now)) throw fail('validation', 'Not a valid time');
};

/**
 * Mirrors `resolve_when`: the Due date and time a pick means. A date is taken as given (past
 * ones are refused); a time without a date means today if still ahead, otherwise tomorrow.
 */
export function resolveWhen(
  date: string | null,
  time: string | null,
  local: LocalNow,
): [string | null, string | null] {
  checkClock(local);
  if (time !== null && !isTime(time)) throw fail('validation', 'Not a valid time');
  if (date !== null) {
    if (epochDay(date) === null) throw fail('validation', 'Not a valid date');
    if (date < local.today) throw fail('validation', 'That date is in the past');
    return [date, time];
  }
  if (time === null) return [null, null];
  return [time >= local.now ? local.today : addDays(local.today, 1), time];
}

/** Mirrors `reset_checklist`: every ticked Checklist item unticked. */
export const resetChecklist = (notes: string): string =>
  notes.replace(/^([^\S\n]*[-*+] )\[[xX]\]/gm, '$1[ ]');

/** Mirrors `RepeatRule::check`: the rule settled for storage, or a validation error. */
function checkRule(rule: RepeatRule): RepeatRule {
  const problem = ruleProblem(rule);
  if (problem) throw fail('validation', problem);
  return settleRule(rule);
}

/** A month or year rule that already keeps to a day of the month. */
const hasDay = (r: RepeatRule) => (r.freq === 'month' || r.freq === 'year') && (r.days ?? 0) !== 0;

/** The Task's Repeat, or none. */
function ruleOf(t: Task): RepeatRule | null {
  if (t.repeat_freq === null || t.repeat_every === null || t.repeat_mode === null) return null;
  return {
    freq: t.repeat_freq,
    every: t.repeat_every,
    days: t.repeat_days,
    mode: t.repeat_mode,
  };
}

/**
 * Mirrors `in_day`: timed Tasks first by time; inside one time block the more urgent Priority
 * goes first; ties keep the manual order.
 */
function inDay(a: Task, b: Task): number {
  if (a.due_time !== null && b.due_time !== null && a.due_time !== b.due_time) {
    return a.due_time < b.due_time ? -1 : 1;
  }
  if ((a.due_time === null) !== (b.due_time === null)) return a.due_time === null ? 1 : -1;
  if (a.priority !== b.priority) return a.priority - b.priority;
  return byKey(a, b);
}
const byDateThenDay = (a: Task, b: Task) =>
  a.due_date === b.due_date ? inDay(a, b) : (a.due_date ?? '') < (b.due_date ?? '') ? -1 : 1;

/** Plain byte-order comparison of keys (what SQLite's default collation does). */
const byKey = (a: Task, b: Task) =>
  a.sort_key < b.sort_key ? -1 : a.sort_key > b.sort_key ? 1 : a.id < b.id ? -1 : 1;
const byListKey = (a: TodoList, b: TodoList) =>
  a.sort_key < b.sort_key ? -1 : a.sort_key > b.sort_key ? 1 : a.id < b.id ? -1 : 1;

/** A Task as the mock stores it: `deleted` marks a soft delete, which Undo clears. */
type Stored = Task & { deleted?: boolean };
type StoredList = TodoList & { deleted?: boolean };
type StoredComment = Comment & { deleted?: boolean };

export function createTodoMock() {
  const all: Stored[] = [];
  const lists: StoredList[] = [];
  const comments: StoredComment[] = [];
  /** The change log, per Task: what History reads (the mock keeps it apart from the Tasks). */
  const logs = new Map<string, LogRecord[]>();
  let seq = 0;
  /** Mirrors `record_in`: a session keeps one record, and a `create` stays a `create`. */
  const record = (
    t: Stored,
    op: LogRecord['op'],
    opts: { session?: string; deletedAt?: number } = {},
  ) => {
    const { deleted: _d, comment_count: _c, ...rest } = t;
    const snapshot = { ...copy(rest), deleted_at: opts.deletedAt ?? null };
    const at = Date.now();
    const session = opts.session ?? null;
    const records = logs.get(t.id) ?? [];
    logs.set(t.id, records);
    const same = session === null ? undefined : records.find((r) => r.session === session);
    if (same) {
      same.snapshot = snapshot;
      same.at = at;
      if (same.op !== 'create') same.op = op;
      return;
    }
    records.push({ op, session, snapshot, at, seq: ++seq });
  };
  const listLabel = (id: string | null) => {
    if (id === null) return '📥 Inbox';
    const list = lists.find((l) => l.id === id);
    return list ? `${list.emoji ?? '≡'} ${list.name}` : 'a deleted List';
  };
  const findComment = (id: unknown) => comments.find((c) => c.id === id && !c.deleted);
  const commentMissing = () => fail('not_found', 'That comment no longer exists');
  const wireComment = (c: StoredComment): Comment => {
    const { deleted: _deleted, ...comment } = c;
    return comment;
  };
  const countComments = (taskId: string) =>
    comments.filter((c) => c.task_id === taskId && !c.deleted).length;
  const live = () => all.filter((t) => !t.deleted);
  const find = (id: unknown) => live().find((t) => t.id === id);
  const missing = () => fail('not_found', 'That Task no longer exists');
  const listMissing = () => fail('not_found', 'That List no longer exists');
  const liveLists = () => lists.filter((l) => !l.deleted).sort(byListKey);
  const findList = (id: unknown) => lists.find((l) => l.id === id && !l.deleted);
  const nameTaken = (name: string, except?: string) =>
    lists.some((l) => !l.deleted && l.id !== except && foldName(l.name) === foldName(name));
  const taken = () => fail('validation', 'There’s already a List with that name');
  const openIn = (listId: string | null) =>
    live()
      .filter((t) => t.list_id === listId && t.state === 'open')
      .sort(byKey);
  const openDated = () => live().filter((t) => t.state === 'open' && t.due_date !== null);
  /**
   * Mirrors `apply`: writes a When and Repeat. No date means no time and no Repeat; a new date is
   * the new anchor of a month or year Repeat unless `keepDay` and the rule has a day already.
   */
  const applyWhen = (
    task: Stored,
    date: string | null,
    time: string | null,
    rule: RepeatRule | null,
    keepDay: boolean,
  ) => {
    let next = rule;
    if (date === null) next = null;
    else if (next && !(keepDay && hasDay(next))) next = anchoredOn(next, date);
    const before = ruleOf(task);
    if (
      task.due_date === date &&
      task.due_time === time &&
      JSON.stringify(before) === JSON.stringify(next)
    ) {
      return;
    }
    task.due_date = date;
    task.due_time = time;
    task.repeat_freq = next?.freq ?? null;
    task.repeat_every = next?.every ?? null;
    task.repeat_days = next?.days ?? null;
    task.repeat_mode = next?.mode ?? null;
    task.updated_at = Date.now();
    record(task, 'update');
  };
  /** A Task without the mock's own flag, like the wire shape. */
  const wire = (t: Stored): Task => {
    const { deleted: _deleted, ...task } = t;
    return { ...task, comment_count: countComments(t.id) };
  };
  const wireList = (l: StoredList): TodoList => {
    const { deleted: _deleted, ...list } = l;
    return list;
  };

  return {
    handles: (cmd: string) => cmd.startsWith('todo_'),

    handle(cmd: string, a: Record<string, unknown>): unknown {
      switch (cmd) {
        case 'todo_inbox': {
          const view: InboxView = { tasks: openIn(null).map(wire) };
          return copy(view);
        }
        case 'todo_task_create': {
          const title = validateTitle(a.title as string);
          const listId = (a.listId as string | null | undefined) ?? null;
          const when = (a.when as NewWhen | null | undefined) ?? null;
          const [dueDate, dueTime] = when
            ? resolveWhen(when.date, when.time, when.local)
            : [null, null];
          if (listId !== null && !findList(listId)) throw listMissing();
          const now = Date.now();
          const last = all.reduce<string | null>(
            (max, t) => (max === null || t.sort_key > max ? t.sort_key : max),
            null,
          );
          const task: Task = {
            id: crypto.randomUUID(),
            list_id: listId,
            title,
            notes: '',
            due_date: dueDate,
            due_time: dueTime,
            repeat_freq: null,
            repeat_every: null,
            repeat_days: null,
            repeat_mode: null,
            priority: 4,
            state: 'open',
            closed_at: null,
            repeat_of: null,
            understood: null,
            sort_key: keyAfter(last),
            created_at: now,
            updated_at: now,
            comment_count: 0,
          };
          all.push(task);
          record(task, 'create');
          return copy(wire(task));
        }
        case 'todo_task_complete': {
          const local = a.local as LocalNow;
          checkClock(local);
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state !== 'open') throw fail('validation', 'That Task is already closed');
          const rule = ruleOf(task);
          if (rule) {
            const due = task.due_date ?? local.today;
            const next = nextDue(rule, due, local.today);
            if (next === null) throw fail('validation', 'That repeat runs past the year 9999');
            const now = Date.now();
            const occurrence: Stored = {
              ...task,
              id: crypto.randomUUID(),
              repeat_freq: null,
              repeat_every: null,
              repeat_days: null,
              repeat_mode: null,
              state: 'done',
              closed_at: now,
              repeat_of: task.id,
              understood: null,
              created_at: now,
              updated_at: now,
            };
            all.push(occurrence);
            record(occurrence, 'create');
            for (const c of comments.filter((c) => c.task_id === task.id))
              c.task_id = occurrence.id;
            task.notes = resetChecklist(task.notes);
            task.due_date = next;
            task.updated_at = now;
            record(task, 'tick');
            const done: Completion = { task: wire(task), occurrence: wire(occurrence) };
            return copy(done);
          }
          task.state = 'done';
          task.closed_at = task.updated_at = Date.now();
          record(task, 'tick');
          const done: Completion = { task: wire(task), occurrence: null };
          return copy(done);
        }
        case 'todo_task_skip': {
          const local = a.local as LocalNow;
          checkClock(local);
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state !== 'open') throw fail('validation', 'That Task is already closed');
          const rule = ruleOf(task);
          if (!rule) throw fail('validation', 'Only a repeating Task can be skipped');
          const next = nextDue(rule, task.due_date ?? local.today, local.today);
          if (next === null) throw fail('validation', 'That repeat runs past the year 9999');
          task.due_date = next;
          task.updated_at = Date.now();
          record(task, 'update');
          return copy(wire(task));
        }
        case 'todo_done': {
          const local = a.local as LocalNow;
          checkClock(local);
          const before = (a.before as string | null | undefined) ?? null;
          if (before !== null && epochDay(before) === null) {
            throw fail('validation', 'Not a valid date');
          }
          const size = Math.min(Math.max(Number(a.days) || 1, 1), 60);
          const dayOf = (ms: number) =>
            new Date(ms + local.offset_min * 60_000).toISOString().slice(0, 10);
          const doneTasks = live().filter(
            (t) => (t.state === 'done' || t.state === 'wont') && t.closed_at !== null,
          );
          const found = [...new Set(doneTasks.map((t) => dayOf(t.closed_at as number)))]
            .filter((d) => before === null || d < before)
            .sort()
            .reverse();
          const kept = found.slice(0, size);
          const newestFirst = (x: Task, y: Task) =>
            (y.closed_at as number) - (x.closed_at as number) || (x.sort_key < y.sort_key ? 1 : -1);
          const view: DoneView = {
            days: kept.map((date) => ({
              date,
              tasks: doneTasks
                .filter((t) => dayOf(t.closed_at as number) === date)
                .sort(newestFirst)
                .map(wire),
            })),
            next_cursor: found.length > size ? (kept[kept.length - 1] ?? null) : null,
          };
          return copy(view);
        }
        case 'todo_task_reopen': {
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state === 'open') throw fail('validation', 'That Task is already open');
          if (task.repeat_of !== null && task.state === 'done') {
            // Mirrors `rewind`: only the latest occurrence, and only while its Task is open.
            const parent = find(task.repeat_of);
            if (!parent) {
              throw fail(
                'validation',
                "This Task was deleted, so its past occurrences can't be reopened",
              );
            }
            // The newest by closing time; copies made in the same millisecond go by creation.
            const latest = live()
              .filter((t) => t.repeat_of === parent.id && t.state === 'done')
              .reduce<Stored | null>(
                (best, t) =>
                  !best || (t.closed_at as number) >= (best.closed_at as number) ? t : best,
                null,
              );
            if (latest?.id !== task.id) {
              throw fail('validation', 'Only the latest occurrence can be reopened');
            }
            if (parent.state !== 'open') {
              throw fail(
                'validation',
                "This Task has changed since, so the occurrence can't be reopened",
              );
            }
            for (const c of comments.filter((c) => c.task_id === task.id)) c.task_id = parent.id;
            if (parent.notes === resetChecklist(task.notes)) parent.notes = task.notes;
            parent.due_date = task.due_date;
            parent.due_time = task.due_time;
            parent.updated_at = Date.now();
            record(parent, 'tick');
            task.deleted = true;
            record(task, 'delete', { deletedAt: Date.now() });
            return copy(wire(parent));
          }
          task.state = 'open';
          task.closed_at = null;
          task.updated_at = Date.now();
          record(task, 'tick');
          return copy(wire(task));
        }
        case 'todo_task_move': {
          const task = find(a.id);
          if (!task) throw missing();
          const listId = (a.listId as string | null | undefined) ?? null;
          if (listId !== null && !findList(listId)) throw listMissing();
          if (task.list_id !== listId) {
            task.list_id = listId;
            task.updated_at = Date.now();
            record(task, 'move');
          }
          return copy(wire(task));
        }
        case 'todo_task_set_when': {
          const local = a.local as LocalNow;
          const given = (a.repeat as RepeatRule | null | undefined) ?? null;
          const rule = given ? checkRule(given) : null;
          const inDate = (a.date as string | null) ?? null;
          const inTime = (a.time as string | null) ?? null;
          let date: string | null;
          let time: string | null;
          if (inDate === null && rule) {
            checkClock(local);
            if (inTime !== null && !isTime(inTime)) throw fail('validation', 'Not a valid time');
            date = firstMatch(rule, local.today);
            time = inTime;
          } else {
            [date, time] = resolveWhen(inDate, inTime, local);
          }
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state !== 'open') throw fail('validation', 'That Task is already closed');
          applyWhen(task, date, time, rule, task.due_date === date);
          return copy(wire(task));
        }
        case 'todo_tasks_restore_when': {
          const changes = a.changes as DueChange[];
          for (const c of changes) {
            if (c.due_date !== null && epochDay(c.due_date) === null) {
              throw fail('validation', 'Not a valid date');
            }
            if (c.due_time !== null && !isTime(c.due_time)) {
              throw fail('validation', 'Not a valid time');
            }
            if (c.due_time !== null && c.due_date === null) {
              throw fail('validation', 'A time needs a date');
            }
          }
          const rules = changes.map((c) => (c.repeat ? checkRule(c.repeat) : null));
          const restored: Task[] = [];
          changes.forEach((c, i) => {
            const task = find(c.id);
            if (!task) return;
            applyWhen(task, c.due_date, c.due_time, rules[i] ?? null, true);
            restored.push(wire(task));
          });
          return copy(restored);
        }
        case 'todo_tasks_move_overdue': {
          const local = a.local as LocalNow;
          const date = a.date as string;
          checkClock(local);
          if (epochDay(date) === null) throw fail('validation', 'Not a valid date');
          if (date < local.today) throw fail('validation', 'That date is in the past');
          const previous: DueChange[] = [];
          for (const task of openDated().filter((t) => (t.due_date as string) < local.today)) {
            const rule = ruleOf(task);
            previous.push({
              id: task.id,
              due_date: task.due_date,
              due_time: task.due_time,
              repeat: rule && settleRule(rule),
            });
            applyWhen(task, date, task.due_time, rule, false);
          }
          const moved: OverdueMove = { to_date: date, previous };
          return copy(moved);
        }
        case 'todo_today': {
          const local = a.local as LocalNow;
          checkClock(local);
          const dated = openDated();
          const overdue = dated.filter((t) => (t.due_date as string) < local.today);
          const tasks = dated.filter((t) => t.due_date === local.today);
          const start = (epochDay(local.today) as number) * DAY_MS - local.offset_min * 60_000;
          const done = live()
            .filter(
              (t) =>
                t.state === 'done' &&
                t.closed_at !== null &&
                t.closed_at >= start &&
                t.closed_at < start + DAY_MS,
            )
            .sort((x, y) => (x.closed_at as number) - (y.closed_at as number) || byKey(x, y));
          const view: TodayView = {
            overdue: overdue.sort(byDateThenDay).map(wire),
            tasks: tasks.sort(inDay).map(wire),
            done: done.map(wire),
            total: overdue.length + tasks.length + done.length,
          };
          return copy(view);
        }
        case 'todo_upcoming': {
          const local = a.local as LocalNow;
          checkClock(local);
          const dated = openDated().sort(byDateThenDay);
          const days: UpcomingView['days'] = [];
          for (const task of dated.filter((t) => (t.due_date as string) > local.today)) {
            const last = days[days.length - 1];
            if (last && last.date === task.due_date) last.tasks.push(wire(task));
            else days.push({ date: task.due_date as string, tasks: [wire(task)] });
          }
          const view: UpcomingView = {
            overdue: dated.filter((t) => (t.due_date as string) < local.today).map(wire),
            days,
          };
          return copy(view);
        }
        case 'todo_day_counts': {
          const from = a.from as string;
          const to = a.to as string;
          if (epochDay(from) === null || epochDay(to) === null) {
            throw fail('validation', 'Not a valid date');
          }
          if (from > to) throw fail('validation', 'The range ends before it starts');
          const counts = new Map<string, number>();
          for (const t of openDated()) {
            const day = t.due_date as string;
            if (day < from || day > to || t.id === a.exclude) continue;
            counts.set(day, (counts.get(day) ?? 0) + 1);
          }
          const out: DayCount[] = [...counts]
            .sort(([x], [y]) => (x < y ? -1 : 1))
            .map(([date, count]) => ({ date, count }));
          return copy(out);
        }
        case 'todo_lists': {
          const local = a.local as LocalNow;
          checkClock(local);
          const dated = openDated();
          const overdue = dated.filter((t) => (t.due_date as string) < local.today).length;
          const dueToday = dated.filter((t) => t.due_date === local.today).length;
          const count = (id: string | null) => openIn(id).length;
          const view: SidebarView = {
            inbox_count: count(null),
            today_count: overdue + dueToday,
            overdue_count: overdue,
            lists: liveLists().map((l) => ({
              list: wireList(l),
              open_count: count(l.id),
              task_count: live().filter((t) => t.list_id === l.id).length,
            })),
          };
          return copy(view);
        }
        case 'todo_list': {
          const list = findList(a.id);
          if (!list) throw listMissing();
          const view: ListView = { list: wireList(list), tasks: openIn(list.id).map(wire) };
          return copy(view);
        }
        case 'todo_all': {
          const groups: AllView['groups'] = [
            { list: null, tasks: openIn(null).map(wire) },
            ...liveLists().map((l) => ({ list: wireList(l), tasks: openIn(l.id).map(wire) })),
          ].filter((g) => g.tasks.length > 0);
          return copy({ groups });
        }
        case 'todo_list_create': {
          const name = validateListName(a.name as string);
          if (nameTaken(name)) throw taken();
          const last = lists.reduce<string | null>(
            (max, l) => (max === null || l.sort_key > max ? l.sort_key : max),
            null,
          );
          const now = Date.now();
          const list: StoredList = {
            id: crypto.randomUUID(),
            name,
            emoji: null,
            sort_key: keyAfter(last),
            created_at: now,
            updated_at: now,
          };
          lists.push(list);
          return copy(wireList(list));
        }
        case 'todo_list_rename': {
          const name = validateListName(a.name as string);
          const list = findList(a.id);
          if (!list) throw listMissing();
          if (nameTaken(name, list.id)) throw taken();
          if (list.name !== name) {
            list.name = name;
            list.updated_at = Date.now();
          }
          return copy(wireList(list));
        }
        case 'todo_list_set_emoji': {
          const emoji = validateEmoji((a.emoji as string | null | undefined) ?? null);
          const list = findList(a.id);
          if (!list) throw listMissing();
          if (list.emoji !== emoji) {
            list.emoji = emoji;
            list.updated_at = Date.now();
          }
          return copy(wireList(list));
        }
        case 'todo_list_delete': {
          const list = findList(a.id);
          if (!list) throw listMissing();
          const choice = a.choice as DeleteChoice;
          let target: string | null = null;
          if (choice.kind === 'move_to_list') {
            if (choice.list_id === list.id) {
              throw fail('validation', 'Pick a different List to move them to');
            }
            if (!findList(choice.list_id)) throw listMissing();
            target = choice.list_id;
          }
          const now = Date.now();
          const deletion: ListDeletion = {
            list_id: list.id,
            deleted_task_ids: [],
            moved_task_ids: [],
          };
          for (const task of live().filter((t) => t.list_id === list.id)) {
            if (choice.kind === 'delete_tasks') {
              record(task, 'delete', { deletedAt: now });
              task.deleted = true;
              deletion.deleted_task_ids.push(task.id);
            } else {
              task.list_id = target;
              task.updated_at = now;
              record(task, 'move');
              deletion.moved_task_ids.push(task.id);
            }
          }
          list.deleted = true;
          list.updated_at = now;
          return copy(deletion);
        }
        case 'todo_list_restore': {
          const deletion = a.deletion as ListDeletion;
          const list = lists.find((l) => l.id === deletion.list_id);
          if (!list) throw listMissing();
          if (!list.deleted) throw fail('validation', "That List isn't deleted");
          if (nameTaken(list.name)) throw taken();
          const now = Date.now();
          list.deleted = false;
          list.updated_at = now;
          for (const id of deletion.deleted_task_ids) {
            const task = all.find((t) => t.id === id);
            if (task) {
              task.deleted = false;
              record(task, 'restore');
            }
          }
          for (const id of deletion.moved_task_ids) {
            const task = find(id);
            if (task) {
              task.list_id = list.id;
              task.updated_at = now;
              record(task, 'move');
            }
          }
          return copy(wireList(list));
        }
        case 'todo_task': {
          const task = find(a.id);
          if (!task) throw missing();
          return copy(wire(task));
        }
        case 'todo_task_update': {
          const title = validateTitle(a.title as string);
          if ([...(a.notes as string)].length > 100_000) {
            throw fail('validation', 'Notes can be at most 100,000 characters');
          }
          const session = a.sessionId as string;
          if (!session.trim()) throw fail('validation', 'An editing session needs an id');
          const task = find(a.id);
          if (!task) throw missing();
          const notes = a.notes as string;
          if (task.title !== title || task.notes !== notes) {
            task.title = title;
            task.notes = notes;
            task.updated_at = Date.now();
            record(task, 'update', { session });
          }
          return copy(wire(task));
        }
        case 'todo_task_set_priority': {
          const priority = a.priority as number;
          if (!Number.isInteger(priority) || priority < 1 || priority > 4) {
            throw fail('validation', 'Priority is 1 to 4');
          }
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state !== 'open') throw fail('validation', 'That Task is already closed');
          if (task.priority !== priority) {
            task.priority = priority;
            task.updated_at = Date.now();
            record(task, 'update');
          }
          return copy(wire(task));
        }
        case 'todo_task_wont_do': {
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state !== 'open') throw fail('validation', 'That Task is already closed');
          if (ruleOf(task)) throw fail('validation', 'A repeating Task is skipped instead');
          task.state = 'wont';
          task.closed_at = task.updated_at = Date.now();
          record(task, 'tick');
          return copy(wire(task));
        }
        case 'todo_task_delete': {
          const task = find(a.id);
          if (!task) throw missing();
          const now = Date.now();
          task.deleted = true;
          task.updated_at = now;
          record(task, 'delete', { deletedAt: now });
          return null;
        }
        case 'todo_task_restore': {
          const task = all.find((t) => t.id === a.id);
          if (!task) throw missing();
          if (!task.deleted) throw fail('validation', "That Task isn't deleted");
          task.deleted = false;
          task.updated_at = Date.now();
          if (task.list_id !== null && !findList(task.list_id)) task.list_id = null;
          record(task, 'restore');
          return copy(wire(task));
        }
        case 'todo_task_history': {
          const task = find(a.id);
          if (!task) throw missing();
          return copy(historyOf(logs.get(task.id) ?? [], listLabel));
        }
        case 'todo_comments': {
          if (!find(a.taskId)) throw missing();
          const own = comments
            .filter((c) => c.task_id === a.taskId && !c.deleted)
            // Ties keep the order they were added in, like Rust's time-ordered ids.
            .sort((x, y) => x.created_at - y.created_at);
          return copy(own.map(wireComment));
        }
        case 'todo_comment_add': {
          const body = validateBody(a.body as string);
          if (!find(a.taskId)) throw missing();
          const now = Date.now();
          const comment: StoredComment = {
            id: crypto.randomUUID(),
            task_id: a.taskId as string,
            body,
            edited: false,
            created_at: now,
            updated_at: now,
          };
          comments.push(comment);
          return copy(wireComment(comment));
        }
        case 'todo_comment_edit': {
          const body = validateBody(a.body as string);
          const comment = findComment(a.id);
          if (!comment) throw commentMissing();
          if (comment.body !== body) {
            comment.body = body;
            comment.updated_at = Math.max(Date.now(), comment.created_at + 1);
            comment.edited = true;
          }
          return copy(wireComment(comment));
        }
        case 'todo_comment_delete': {
          const comment = findComment(a.id);
          if (!comment) throw commentMissing();
          comment.deleted = true;
          return null;
        }
        case 'todo_comment_restore': {
          const comment = comments.find((c) => c.id === a.id);
          if (!comment) throw commentMissing();
          if (!comment.deleted) throw fail('validation', "That comment isn't deleted");
          comment.deleted = false;
          return copy(wireComment(comment));
        }
        default:
          return undefined;
      }
    },
  };
}
