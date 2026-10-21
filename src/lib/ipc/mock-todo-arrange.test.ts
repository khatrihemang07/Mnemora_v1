/** The mock's grouping and sorting, with the same cases as the unit tests in arrange.rs. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ListEntry, ListView, LocalNow, Task, TodoList } from './bindings';
import { createTodoMock } from './mock-todo';
import {
  compareTasks,
  dueBucket,
  groupTasks,
  PREF_VALUES,
  readPrefs,
  sortLists,
} from './mock-todo-arrange';

const TODAY = '2026-10-21';

function task(
  title: string,
  key: string,
  priority: number,
  due: string | null,
  list: string | null,
) {
  return {
    id: `id-${title}`,
    list_id: list,
    title,
    notes: '',
    due_date: due,
    due_time: null,
    repeat_freq: null,
    repeat_every: null,
    repeat_days: null,
    repeat_mode: null,
    priority,
    state: 'open',
    closed_at: null,
    repeat_of: null,
    understood: null,
    sort_key: key,
    created_at: 0,
    updated_at: 0,
    comment_count: 0,
  } satisfies Task;
}

const titles = (groups: { tasks: Task[] }[]) => groups.map((g) => g.tasks.map((t) => t.title));

describe('arranging in the mock', () => {
  it('puts due dates in buckets', () => {
    const at = (d: string | null) => dueBucket(d, TODAY);
    expect(at(null)).toBe('no_date');
    expect(at('2026-10-20')).toBe('overdue');
    expect(at('2026-10-21')).toBe('today');
    expect(at('2026-10-22')).toBe('tomorrow');
    expect(at('2026-10-23')).toBe('week');
    expect(at('2026-10-28')).toBe('week');
    expect(at('2026-10-29')).toBe('later');
  });

  it('orders by each sort and breaks ties by the drag order', () => {
    const a = { ...task('b', '3', 2, '2026-10-25', null), created_at: 10 };
    const b = { ...task('A', '1', 4, null, null), created_at: 30 };
    const c = { ...task('c', '2', 2, '2026-10-22', null), created_at: 20 };
    const order = (sort: Parameters<typeof compareTasks>[0]) =>
      [a, b, c].sort(compareTasks(sort)).map((t) => t.title);
    expect(order('manual')).toEqual(['A', 'c', 'b']);
    expect(order('due')).toEqual(['c', 'b', 'A']);
    expect(order('priority')).toEqual(['c', 'b', 'A']);
    expect(order('title')).toEqual(['A', 'b', 'c']);
    expect(order('added')).toEqual(['A', 'c', 'b']);
  });

  it('groups first, then sorts inside each group', () => {
    const home = { id: 'L1', name: 'Home' } as TodoList;
    const ctx = { lists: [home], today: TODAY };
    const tasks = [
      task('a', '1', 3, null, null),
      task('b', '2', 1, '2026-10-21', 'L1'),
      task('c', '3', 3, '2026-10-30', 'L1'),
      task('d', '4', 1, '2026-10-20', null),
    ];
    const cmp = compareTasks('manual');
    const by = (g: Parameters<typeof groupTasks>[1]) => groupTasks(tasks, g, ctx, cmp);
    expect(titles(by('none'))).toEqual([['a', 'b', 'c', 'd']]);
    const p = by('priority');
    expect(titles(p)).toEqual([
      ['b', 'd'],
      ['a', 'c'],
    ]);
    expect(p[0]?.key).toEqual({ kind: 'priority', priority: 1 });
    const l = by('list');
    expect(titles(l)).toEqual([
      ['a', 'd'],
      ['b', 'c'],
    ]);
    expect(l[0]?.key).toEqual({ kind: 'list', list_id: null });
    const d = by('due');
    expect(titles(d)).toEqual([['d'], ['b'], ['c'], ['a']]);
    expect(d[1]?.adopt).toEqual([{ kind: 'day', date: TODAY }]);
    expect(d[0]?.adopt).toEqual([]);
    const days = by('day');
    expect(titles(days)).toEqual([['d'], ['b'], ['c'], ['a']]);
    expect(days[3]?.key).toEqual({ kind: 'plain' });
  });

  it('sorts Lists by name, open count and creation, keeping the drag order for ties', () => {
    const entry = (name: string, open: number, created: number): ListEntry => ({
      list: { id: name, name, created_at: created, sort_key: name } as TodoList,
      open_count: open,
      task_count: open,
    });
    const base = [entry('beta', 2, 3), entry('Álpha', 5, 2), entry('gamma', 2, 1)];
    const names = (sort: Parameters<typeof sortLists>[1]) =>
      sortLists(base, sort).map((e) => e.list.name);
    expect(names('manual')).toEqual(['beta', 'Álpha', 'gamma']);
    expect(names('name_asc')).toEqual(['Álpha', 'beta', 'gamma']);
    expect(names('name_desc')).toEqual(['gamma', 'beta', 'Álpha']);
    expect(names('most_open')).toEqual(['Álpha', 'beta', 'gamma']);
    expect(names('created')).toEqual(['gamma', 'Álpha', 'beta']);
  });

  it('reads Preferences, falling back to the default for anything unknown', () => {
    const defaults = readPrefs(() => undefined);
    expect(defaults).toEqual({
      group_today: 'priority',
      group_upcoming: 'day',
      group_inbox: 'none',
      group_all: 'list',
      inbox_sort: 'manual',
      lists_sort: 'manual',
      done_in_lists: 'today_shown',
    });
    const saved = new Map<string, unknown>([
      ['todo.group_today', 'list'],
      ['todo.group_inbox', 'bogus'],
      ['todo.lists_sort', 7],
    ]);
    const read = readPrefs((k) => saved.get(k));
    expect([read.group_today, read.group_inbox, read.lists_sort]).toEqual([
      'list',
      'none',
      'manual',
    ]);
    // The mock's Settings accept exactly these values.
    expect(PREF_VALUES.group_all).toEqual(['list', 'priority', 'due', 'none']);
  });
});

describe('Done Tasks in a List, in the mock', () => {
  afterEach(() => vi.useRealTimers());

  /** A Home List with Tasks done today, yesterday and last week, and one Won't do today. */
  function setup(mode: string) {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-10-21T10:00:00Z'));
    const mock = createTodoMock((k) => (k === 'todo.done_in_lists' ? mode : undefined));
    const home = mock.handle('todo_list_create', { name: 'Home' }) as TodoList;
    const mk = (title: string) =>
      mock.handle('todo_task_create', { title, listId: home.id }) as Task;
    mk('open');
    const close = (title: string, at: string, wont = false) => {
      const { id } = mk(title);
      vi.setSystemTime(new Date(at));
      mock.handle(wont ? 'todo_task_wont_do' : 'todo_task_complete', {
        id,
        local: { today: '2026-10-21', now: '10:00', offset_min: 0 },
      });
    };
    close('done last week', '2026-10-14T10:00:00Z');
    close('done yesterday', '2026-10-20T10:00:00Z');
    close('done today', '2026-10-21T08:00:00Z');
    close('wont today', '2026-10-21T09:00:00Z', true);
    const local: LocalNow = { today: '2026-10-21', now: '10:00', offset_min: 0 };
    return mock.handle('todo_list', { id: home.id, local }) as ListView;
  }
  const names = (ts: Task[]) => ts.map((t) => t.title);

  it('shows done today and folds the older ones by default', () => {
    const view = setup('today_shown');
    expect(names(view.done.shown)).toEqual(['wont today', 'done today']);
    expect(names(view.done.folded)).toEqual(['done yesterday', 'done last week']);
    expect(view.groups[0]?.tasks.map((t) => t.title)).toEqual(['open']);
  });

  it('folds, shows or hides all of them', () => {
    const folded = setup('folded');
    expect([folded.done.shown.length, folded.done.folded.length]).toEqual([0, 4]);
    const shown = setup('shown');
    expect([shown.done.shown.length, shown.done.folded.length]).toEqual([4, 0]);
    const hidden = setup('hidden');
    expect([hidden.done.mode, hidden.done.shown.length, hidden.done.folded.length]).toEqual([
      'hidden',
      0,
      0,
    ]);
  });
});
