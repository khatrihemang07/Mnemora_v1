/**
 * Grouping, sorting and the view Preferences for the browser mock. Mirrors
 * crates/todo/src/arrange.rs; the shared rule fixtures keep the two in step (ADR 0009).
 */

import { addDays } from '../ui/calendar';
import type {
  DoneInLists,
  DueBucket,
  GroupBy,
  GroupKey,
  ListEntry,
  ListSort,
  Task,
  TaskGroup,
  TaskSort,
  TodoList,
} from './bindings';
import { foldName } from './mock-todo';

/** Mirrors the Preference keys in arrange.rs: the first value of each is its default. */
export const PREF_VALUES = {
  group_today: ['priority', 'none', 'list'],
  group_upcoming: ['day', 'priority', 'list'],
  group_inbox: ['none', 'priority', 'due'],
  group_all: ['list', 'priority', 'due', 'none'],
  inbox_sort: ['manual', 'due', 'priority', 'title', 'added'],
  lists_sort: ['manual', 'name_asc', 'name_desc', 'most_open', 'created'],
  done_in_lists: ['today_shown', 'folded', 'shown', 'hidden'],
} as const;

/** The Preferences the views read. */
export interface ViewPrefs {
  group_today: GroupBy;
  group_upcoming: GroupBy;
  group_inbox: GroupBy;
  group_all: GroupBy;
  inbox_sort: TaskSort;
  lists_sort: ListSort;
  done_in_lists: DoneInLists;
}

/** Mirrors `ViewPrefs::read`: a missing or unknown value is the default. */
export function readPrefs(get: (key: string) => unknown): ViewPrefs {
  const one = (key: keyof typeof PREF_VALUES) => {
    const saved = get(`todo.${key}`);
    const allowed: readonly string[] = PREF_VALUES[key];
    return typeof saved === 'string' && allowed.includes(saved) ? saved : allowed[0];
  };
  return {
    group_today: one('group_today') as GroupBy,
    group_upcoming: one('group_upcoming') as GroupBy,
    group_inbox: one('group_inbox') as GroupBy,
    group_all: one('group_all') as GroupBy,
    inbox_sort: one('inbox_sort') as TaskSort,
    lists_sort: one('lists_sort') as ListSort,
    done_in_lists: one('done_in_lists') as DoneInLists,
  };
}

const byKey = (a: Task, b: Task): number =>
  a.sort_key < b.sort_key ? -1 : a.sort_key > b.sort_key ? 1 : a.id < b.id ? -1 : 1;

/** Earlier values first, none last. */
function undatedLast(a: string | null, b: string | null): number {
  if (a !== null && b !== null) return a < b ? -1 : a > b ? 1 : 0;
  if (a !== null) return -1;
  if (b !== null) return 1;
  return 0;
}

/** Mirrors `compare`: a sort's order; ties fall back to the manual order. */
export function compareTasks(sort: TaskSort): (a: Task, b: Task) => number {
  return (a, b) => {
    let primary = 0;
    if (sort === 'due') {
      primary = undatedLast(a.due_date, b.due_date) || undatedLast(a.due_time, b.due_time);
    } else if (sort === 'priority') {
      primary = a.priority - b.priority;
    } else if (sort === 'title') {
      const x = foldName(a.title);
      const y = foldName(b.title);
      primary = x < y ? -1 : x > y ? 1 : 0;
    } else if (sort === 'added') {
      primary =
        b.created_at - a.created_at ||
        (b.sort_key < a.sort_key ? -1 : b.sort_key > a.sort_key ? 1 : 0);
    }
    return primary || byKey(a, b);
  };
}

/** Mirrors `due_bucket`. */
export function dueBucket(due: string | null, today: string): DueBucket {
  if (due === null) return 'no_date';
  if (due < today) return 'overdue';
  if (due === today) return 'today';
  if (due === addDays(today, 1)) return 'tomorrow';
  return due <= addDays(today, 7) ? 'week' : 'later';
}

const DUE_ORDER: DueBucket[] = ['overdue', 'today', 'tomorrow', 'week', 'later', 'no_date'];

/** Mirrors `group_tasks`: groups, then each sorted by `cmp`; empty groups left out. */
export function groupTasks(
  tasks: Task[],
  by: GroupBy,
  ctx: { lists: TodoList[]; today: string },
  cmp: (a: Task, b: Task) => number,
): TaskGroup[] {
  const sorted = [...tasks].sort(cmp);
  if (by === 'none') return [{ key: { kind: 'plain' }, adopt: [], tasks: sorted }];
  const groups: TaskGroup[] = [];
  const push = (key: GroupKey, adopt: GroupKey[], members: Task[]) => {
    if (members.length > 0) groups.push({ key, adopt, tasks: members });
  };
  if (by === 'priority') {
    for (let priority = 1; priority <= 4; priority++) {
      const key: GroupKey = { kind: 'priority', priority };
      push(
        key,
        [key],
        sorted.filter((t) => t.priority === priority),
      );
    }
  } else if (by === 'list') {
    for (const list_id of [null, ...ctx.lists.map((l) => l.id)]) {
      const key: GroupKey = { kind: 'list', list_id };
      push(
        key,
        [key],
        sorted.filter((t) => t.list_id === list_id),
      );
    }
  } else if (by === 'day') {
    const dates = [...new Set(sorted.flatMap((t) => (t.due_date === null ? [] : [t.due_date])))];
    dates.sort();
    for (const date of dates) {
      const key: GroupKey = { kind: 'day', date };
      push(
        key,
        [key],
        sorted.filter((t) => t.due_date === date),
      );
    }
    push(
      { kind: 'plain' },
      [],
      sorted.filter((t) => t.due_date === null),
    );
  } else {
    for (const bucket of DUE_ORDER) {
      const adopt: GroupKey[] =
        bucket === 'today'
          ? [{ kind: 'day', date: ctx.today }]
          : bucket === 'tomorrow'
            ? [{ kind: 'day', date: addDays(ctx.today, 1) }]
            : [];
      push(
        { kind: 'due', bucket },
        adopt,
        sorted.filter((t) => dueBucket(t.due_date, ctx.today) === bucket),
      );
    }
  }
  return groups;
}

/** Mirrors `sort_lists`: ties (and Manual) keep the drag order `entries` arrive in. */
export function sortLists(entries: ListEntry[], sort: ListSort): ListEntry[] {
  const out = [...entries];
  const name = (e: ListEntry) => foldName(e.list.name);
  if (sort === 'name_asc') out.sort((a, b) => (name(a) < name(b) ? -1 : name(a) > name(b) ? 1 : 0));
  else if (sort === 'name_desc')
    out.sort((a, b) => (name(a) < name(b) ? 1 : name(a) > name(b) ? -1 : 0));
  else if (sort === 'most_open') out.sort((a, b) => b.open_count - a.open_count);
  else if (sort === 'created') {
    // Lists made in the same millisecond keep the order they were added in.
    out.sort(
      (a, b) =>
        a.list.created_at - b.list.created_at ||
        (a.list.sort_key < b.list.sort_key ? -1 : a.list.sort_key > b.list.sort_key ? 1 : 0),
    );
  }
  return out;
}
