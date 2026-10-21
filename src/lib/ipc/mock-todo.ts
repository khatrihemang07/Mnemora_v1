/**
 * In-memory Todo for the browser-only IPC mock. Mirrors the rules in crates/todo (titles are
 * non-blank, one line and at most 500 characters; new Tasks go last; complete and reopen; Lists
 * with folded-unique names, delete choices with restore, and moving Tasks). Every
 * result is a JSON copy, like real IPC: handing out live objects would let the UI mutate the
 * "database". The shared rule fixtures (fixtures/todo-rules.json) keep it in step with Rust.
 */

import type {
  AllView,
  AppError,
  DeleteChoice,
  InboxView,
  ListDeletion,
  ListView,
  SidebarView,
  Task,
  TodoList,
} from './bindings';

const MAX_TITLE_CHARS = 500;
const MAX_LIST_NAME_CHARS = 60;
const MAX_EMOJI_CHARS = 16;

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

/** Plain byte-order comparison of keys (what SQLite's default collation does). */
const byKey = (a: Task, b: Task) =>
  a.sort_key < b.sort_key ? -1 : a.sort_key > b.sort_key ? 1 : a.id < b.id ? -1 : 1;
const byListKey = (a: TodoList, b: TodoList) =>
  a.sort_key < b.sort_key ? -1 : a.sort_key > b.sort_key ? 1 : a.id < b.id ? -1 : 1;

/** A Task as the mock stores it: `deleted` marks a soft delete, which Undo clears. */
type Stored = Task & { deleted?: boolean };
type StoredList = TodoList & { deleted?: boolean };

export function createTodoMock() {
  const all: Stored[] = [];
  const lists: StoredList[] = [];
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
  /** A Task without the mock's own flag, like the wire shape. */
  const wire = (t: Stored): Task => {
    const { deleted: _deleted, ...task } = t;
    return task;
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
          return copy(wire(task));
        }
        case 'todo_task_complete': {
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state !== 'open') throw fail('validation', 'That Task is already closed');
          task.state = 'done';
          task.closed_at = task.updated_at = Date.now();
          return copy(wire(task));
        }
        case 'todo_task_reopen': {
          const task = find(a.id);
          if (!task) throw missing();
          if (task.state === 'open') throw fail('validation', 'That Task is already open');
          task.state = 'open';
          task.closed_at = null;
          task.updated_at = Date.now();
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
          }
          return copy(wire(task));
        }
        case 'todo_lists': {
          const count = (id: string | null) => openIn(id).length;
          const view: SidebarView = {
            inbox_count: count(null),
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
              task.deleted = true;
              deletion.deleted_task_ids.push(task.id);
            } else {
              task.list_id = target;
              task.updated_at = now;
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
            if (task) task.deleted = false;
          }
          for (const id of deletion.moved_task_ids) {
            const task = find(id);
            if (task) {
              task.list_id = list.id;
              task.updated_at = now;
            }
          }
          return copy(wireList(list));
        }
        default:
          return undefined;
      }
    },
  };
}
