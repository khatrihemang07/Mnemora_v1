/**
 * In-memory Todo for the browser-only IPC mock. Mirrors the rules in crates/todo (titles are
 * non-blank, one line and at most 500 characters; new Tasks go last; complete and reopen). Every
 * result is a JSON copy, like real IPC: handing out live objects would let the UI mutate the
 * "database". The shared rule fixtures (fixtures/todo-rules.json) keep it in step with Rust.
 */

import type { AppError, InboxView, Task } from './bindings';

const MAX_TITLE_CHARS = 500;

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

/** Plain byte-order comparison of keys (what SQLite's default collation does). */
const byKey = (a: Task, b: Task) =>
  a.sort_key < b.sort_key ? -1 : a.sort_key > b.sort_key ? 1 : a.id < b.id ? -1 : 1;

export function createTodoMock() {
  const all: Task[] = [];
  const find = (id: unknown) => all.find((t) => t.id === id);
  const missing = () => fail('not_found', 'That Task no longer exists');

  return {
    handles: (cmd: string) => cmd.startsWith('todo_'),

    handle(cmd: string, a: Record<string, unknown>): unknown {
      switch (cmd) {
        case 'todo_inbox': {
          const view: InboxView = {
            tasks: all.filter((t) => t.list_id === null && t.state === 'open').sort(byKey),
          };
          return copy(view);
        }
        case 'todo_task_create': {
          const title = validateTitle(a.title as string);
          const now = Date.now();
          const last = all.reduce<string | null>(
            (max, t) => (max === null || t.sort_key > max ? t.sort_key : max),
            null,
          );
          const task: Task = {
            id: crypto.randomUUID(),
            list_id: null,
            title,
            notes: '',
            due_date: null,
            due_time: null,
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
          };
          all.push(task);
          return copy(task);
        }
        case 'todo_task_complete': {
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state !== 'open') throw fail('validation', 'That Task is already closed');
          task.state = 'done';
          task.closed_at = task.updated_at = Date.now();
          return copy(task);
        }
        case 'todo_task_reopen': {
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state === 'open') throw fail('validation', 'That Task is already open');
          task.state = 'open';
          task.closed_at = null;
          task.updated_at = Date.now();
          return copy(task);
        }
        default:
          return undefined;
      }
    },
  };
}
