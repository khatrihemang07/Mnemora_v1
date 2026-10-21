/**
 * Runs the shared rule fixtures (fixtures/todo-rules.json) against the browser mock. The Rust
 * commands run the same file in src-tauri/src/todo.rs, so the mock cannot drift from them
 * (ADR 0009). Keep `run` in step with `run_rule_case` there.
 */
import { describe, expect, it, vi } from 'vitest';
import type {
  AllView,
  AppError,
  DayCount,
  ListDeletion,
  LocalNow,
  OverdueMove,
  SidebarView,
  Task,
  TodayView,
  TodoList,
  UpcomingView,
} from './bindings';
import rules from './fixtures/todo-rules.json';
import {
  createTodoMock,
  foldName,
  keyAfter,
  validateEmoji,
  validateListName,
  validateTitle,
} from './mock-todo';

interface Step {
  op:
    | 'create'
    | 'complete'
    | 'reopen'
    | 'move'
    | 'list_create'
    | 'list_rename'
    | 'list_emoji'
    | 'list_delete'
    | 'list_restore'
    | 'set_when'
    | 'move_overdue'
    | 'undo_move';
  title?: string;
  date?: string | null;
  time?: string | null;
  today?: string;
  now?: string;
  move?: number;
  name?: string;
  emoji?: string;
  task?: number | string;
  list?: number | string | null;
  choice?: 'delete' | 'inbox' | { list: number | string };
  deletion?: number;
}
interface Case {
  today?: string;
  now?: string;
  rule: string;
  name: string;
  steps: Step[];
  expect: Record<string, unknown>;
}

/** A number picks the n-th created item, null stays null, a string is used as an id. */
const pick = <T>(v: number | string | null | undefined, made: T[]) =>
  typeof v === 'number' ? made[v] : (v ?? null);

const DEFAULT_TODAY = '2026-10-21';
const DEFAULT_NOW = '14:05';

function run(c: Case) {
  const mock = createTodoMock();
  const tasks: string[] = [];
  const lists: string[] = [];
  const deletions: ListDeletion[] = [];
  const moves: OverdueMove[] = [];
  // The clock steps run against: the case's, unless a step pins its own.
  const clock = (step?: Step): LocalNow => ({
    today: step?.today ?? c.today ?? DEFAULT_TODAY,
    now: step?.now ?? c.now ?? DEFAULT_NOW,
    offset_min: 0,
  });
  const results: string[] = [];
  for (const step of c.steps) {
    const id = pick(step.task, tasks);
    const list = pick(step.list, lists);
    try {
      switch (step.op) {
        case 'create': {
          const dated = step.date != null || step.time != null;
          const when = dated
            ? { date: step.date ?? null, time: step.time ?? null, local: clock(step) }
            : null;
          tasks.push(
            (mock.handle('todo_task_create', { title: step.title, listId: list, when }) as Task).id,
          );
          break;
        }
        case 'set_when':
          mock.handle('todo_task_set_when', {
            id,
            date: step.date ?? null,
            time: step.time ?? null,
            local: clock(step),
          });
          break;
        case 'move_overdue':
          moves.push(
            mock.handle('todo_tasks_move_overdue', {
              date: step.date,
              local: clock(step),
            }) as OverdueMove,
          );
          break;
        case 'undo_move':
          mock.handle('todo_tasks_restore_when', { changes: moves[step.move ?? 0]?.previous });
          break;
        case 'move':
          mock.handle('todo_task_move', { id, listId: list });
          break;
        case 'list_create':
          lists.push((mock.handle('todo_list_create', { name: step.name }) as TodoList).id);
          break;
        case 'list_rename':
          mock.handle('todo_list_rename', { id: list, name: step.name });
          break;
        case 'list_emoji':
          mock.handle('todo_list_set_emoji', { id: list, emoji: step.emoji });
          break;
        case 'list_delete': {
          const c = step.choice;
          const choice =
            c === 'delete'
              ? { kind: 'delete_tasks' }
              : c === 'inbox' || c === undefined
                ? { kind: 'move_to_inbox' }
                : { kind: 'move_to_list', list_id: pick(c.list, lists) };
          deletions.push(mock.handle('todo_list_delete', { id: list, choice }) as ListDeletion);
          break;
        }
        case 'list_restore':
          mock.handle('todo_list_restore', { deletion: deletions[step.deletion ?? 0] });
          break;
        default:
          mock.handle(`todo_task_${step.op}`, { id });
      }
      results.push('ok');
    } catch (e) {
      results.push((e as AppError).code);
    }
  }
  const inbox = mock.handle('todo_inbox', {}) as { tasks: Task[] };
  const sidebar = mock.handle('todo_lists', { local: clock() }) as SidebarView;
  const all = mock.handle('todo_all', {}) as AllView;
  const view = clock();
  const titles = (ts: Task[]) => ts.map((t) => t.title);
  const todayView = mock.handle('todo_today', { local: view }) as TodayView;
  const upcoming = mock.handle('todo_upcoming', { local: view }) as UpcomingView;
  const counts = mock.handle('todo_day_counts', {
    from: view.today,
    to: '2027-12-31',
    exclude: null,
  }) as DayCount[];
  return {
    results,
    dues: all.groups.flatMap((g) => g.tasks.map((t) => [t.title, t.due_date, t.due_time])),
    today: { overdue: titles(todayView.overdue), tasks: titles(todayView.tasks) },
    upcoming: {
      overdue: titles(upcoming.overdue),
      days: upcoming.days.map((d) => [d.date, titles(d.tasks)]),
    },
    day_counts: counts.map((d) => [d.date, d.count]),
    inbox: inbox.tasks.map((t) => t.title),
    lists: sidebar.lists.map((e) => e.list.name),
    emojis: sidebar.lists.map((e) => e.list.emoji),
    sidebar_today: [sidebar.today_count, sidebar.overdue_count],
    counts: [sidebar.inbox_count, ...sidebar.lists.map((e) => e.open_count)],
    all: all.groups.map((g) => [g.list?.name ?? 'Inbox', g.tasks.map((t) => t.title)]),
  };
}

describe('the Todo mock follows the shared rule fixtures', () => {
  for (const c of rules.cases as Case[]) {
    it(`${c.rule}: ${c.name}`, () => {
      const actual = run(c);
      const picked = Object.fromEntries(
        Object.keys(c.expect).map((k) => [k, actual[k as keyof typeof actual]]),
      );
      expect(picked).toEqual(c.expect);
    });
  }
});

describe('the Todo mock', () => {
  it("lists what was done today in the device's day", () => {
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date('2026-10-21T08:00:00Z'));
      const mock = createTodoMock();
      const local = { today: '2026-10-21', now: '13:30', offset_min: 330 };
      const mk = (title: string) => mock.handle('todo_task_create', { title }) as Task;
      const [a, b, c] = [mk('a'), mk('b'), mk('c')];
      for (const t of [a, b, c]) {
        mock.handle('todo_task_set_when', { id: t.id, date: '2026-10-21', time: null, local });
      }
      mock.handle('todo_task_complete', { id: a.id });
      vi.setSystemTime(new Date('2026-10-20T18:29:59Z')); // a minute before Kolkata's midnight
      mock.handle('todo_task_complete', { id: b.id });
      const view = mock.handle('todo_today', { local }) as TodayView;
      expect(view.done.map((t) => t.title)).toEqual(['a']);
      expect(view.tasks.map((t) => t.title)).toEqual(['c']);
      expect(view.total).toBe(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it('leaves the excluded Task out of the day counts', () => {
    const mock = createTodoMock();
    const local = { today: '2026-10-21', now: '13:30', offset_min: 0 };
    const [a, b] = ['a', 'b'].map((title) => mock.handle('todo_task_create', { title }) as Task);
    for (const t of [a, b]) {
      mock.handle('todo_task_set_when', { id: t?.id, date: '2026-10-23', time: null, local });
    }
    const range = { from: '2026-10-21', to: '2026-10-30' };
    expect(mock.handle('todo_day_counts', { ...range, exclude: null })).toEqual([
      { date: '2026-10-23', count: 2 },
    ]);
    expect(mock.handle('todo_day_counts', { ...range, exclude: a?.id })).toEqual([
      { date: '2026-10-23', count: 1 },
    ]);
  });

  it('hands out copies, like real IPC', () => {
    const mock = createTodoMock();
    const task = mock.handle('todo_task_create', { title: 'a' }) as Task;
    task.title = 'changed';
    const view = mock.handle('todo_inbox', {}) as { tasks: Task[] };
    expect(view.tasks[0]?.title).toBe('a');
  });

  it('sets and clears the done time', () => {
    const mock = createTodoMock();
    const { id } = mock.handle('todo_task_create', { title: 'a' }) as Task;
    const done = mock.handle('todo_task_complete', { id }) as Task;
    expect(done.state).toBe('done');
    expect(done.closed_at).toBeTypeOf('number');
    const back = mock.handle('todo_task_reopen', { id }) as Task;
    expect([back.state, back.closed_at]).toEqual(['open', null]);
  });

  it('counts characters, not UTF-16 units, for the length limit', () => {
    expect(validateTitle('😀'.repeat(500))).toHaveLength(1000);
    expect(() => validateTitle('😀'.repeat(501))).toThrow();
  });

  it('makes sort keys that always sort later, in step with Rust', () => {
    expect(keyAfter(null)).toBe('V0000000');
    expect(keyAfter('0000000z')).toBe('00000010');
    expect(keyAfter('zzzzzzzz') > 'zzzzzzzz').toBe(true);
    let key = keyAfter(null);
    for (let i = 0; i < 200; i++) {
      const next = keyAfter(key);
      expect(next > key).toBe(true);
      key = next;
    }
  });

  it('folds names like core: case and accents, plus letters that do not decompose', () => {
    expect(foldName('Café')).toBe('cafe');
    expect(foldName('NAÏVE')).toBe('naive');
    expect(foldName('cafe\u0301')).toBe('cafe');
    expect(foldName('Łódź')).toBe('lodz');
  });

  it('counts every Task in a List, done ones too, apart from the open count', () => {
    const mock = createTodoMock();
    const list = mock.handle('todo_list_create', { name: 'Home' }) as TodoList;
    const a = mock.handle('todo_task_create', { title: 'a', listId: list.id }) as Task;
    mock.handle('todo_task_create', { title: 'b', listId: list.id });
    mock.handle('todo_task_complete', { id: a.id });
    const entry = (
      mock.handle('todo_lists', {
        local: { today: DEFAULT_TODAY, now: DEFAULT_NOW, offset_min: 0 },
      }) as SidebarView
    ).lists[0];
    expect([entry?.open_count, entry?.task_count]).toEqual([1, 2]);
  });

  it('validates List names and emoji like Rust', () => {
    expect(validateListName('  Home ')).toBe('Home');
    expect(() => validateListName('é'.repeat(61))).toThrow();
    expect(validateEmoji(' 🏠 ')).toBe('🏠');
    expect(validateEmoji('  ')).toBeNull();
    expect(validateEmoji('👨‍👩‍👧')).toBe('👨‍👩‍👧');
    expect(() => validateEmoji('ab')).toThrow();
    expect(() => validateEmoji('🏠'.repeat(17))).toThrow();
  });
});
