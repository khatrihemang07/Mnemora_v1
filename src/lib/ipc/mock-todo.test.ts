/**
 * Runs the shared rule fixtures (fixtures/todo-rules.json) against the browser mock. The Rust
 * commands run the same file in src-tauri/src/todo.rs, so the mock cannot drift from them
 * (ADR 0009). Keep `run` in step with `run_rule_case` there.
 */
import { describe, expect, it, vi } from 'vitest';
import type {
  AllView,
  AppError,
  Comment,
  Completion,
  DayCount,
  DoneView,
  GroupKey,
  HistoryLine,
  InboxView,
  ListDeletion,
  ListView,
  LocalNow,
  OverdueMove,
  SidebarView,
  Task,
  TaskGroup,
  TodayView,
  TodoList,
  UpcomingView,
} from './bindings';
import orderRules from './fixtures/todo-order-rules.json';
import rules from './fixtures/todo-rules.json';
import {
  createTodoMock,
  foldName,
  resetChecklist,
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
    | 'undo_move'
    | 'skip'
    | 'reopen_occurrence'
    | 'wont_do'
    | 'delete'
    | 'restore'
    | 'priority'
    | 'update'
    | 'comment_add'
    | 'comment_edit'
    | 'comment_delete'
    | 'comment_restore'
    | 'pref'
    | 'reorder'
    | 'list_reorder'
    | 'list_sort'
    | 'list_group';
  key?: string;
  value?: string;
  before?: number | string | null;
  after?: number | string | null;
  target?: { kind: string; list?: number | null; [k: string]: unknown }[];
  sort?: string;
  group_by?: string;
  priority?: number;
  notes?: string;
  session?: string;
  body?: string;
  comment?: number | string;
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
  repeat?: FixtureRepeat | null;
  occurrence?: number;
}
interface FixtureRepeat {
  freq: string;
  every: number;
  mode: string;
  weekdays?: number[];
  anchor?: number;
}
interface Case {
  /** Which created Task's History the case reads (default the first). */
  history_of?: number;
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

/** Mirrors `group_label` in the Rust runner: a group's name in the fixtures. */
function groupLabel(key: GroupKey, listNames: Map<string, string>): string {
  switch (key.kind) {
    case 'priority':
      return `p${key.priority}`;
    case 'list':
      return `list:${key.list_id === null ? 'Inbox' : (listNames.get(key.list_id) ?? '?')}`;
    case 'day':
      return `day:${key.date}`;
    case 'due':
      return `due:${key.bucket}`;
    default:
      return key.kind;
  }
}

/** The Tasks of a view's groups, Overdue left out; and the Overdue ones. */
const tasksOf = (v: { groups: TaskGroup[] }) =>
  v.groups.filter((g) => g.key.kind !== 'overdue').flatMap((g) => g.tasks);
const overdueOf = (v: { groups: TaskGroup[] }) =>
  v.groups.filter((g) => g.key.kind === 'overdue').flatMap((g) => g.tasks);

/** A fixture's Repeat (`weekdays` or `anchor` instead of the wire's `days`) as the wire shape. */
const wireRepeat = (r: FixtureRepeat | null | undefined) =>
  r
    ? {
        freq: r.freq,
        every: r.every,
        days:
          r.freq === 'week'
            ? (r.weekdays ?? []).reduce((m, d) => m | (1 << d), 0)
            : (r.anchor ?? 0),
        mode: r.mode,
      }
    : null;

const DEFAULT_TODAY = '2026-10-21';
const DEFAULT_NOW = '14:05';
const LOCAL: LocalNow = { today: DEFAULT_TODAY, now: DEFAULT_NOW, offset_min: 0 };

function run(c: Case) {
  // The cases predate grouping: they read Today flat unless they choose otherwise.
  const prefs = new Map<string, unknown>([['todo.group_today', 'none']]);
  const mock = createTodoMock((key) => prefs.get(key));
  const reorderOps: string[][] = [];
  const tasks: string[] = [];
  const lists: string[] = [];
  const deletions: ListDeletion[] = [];
  const moves: OverdueMove[] = [];
  const occurrences: string[] = [];
  const comments: string[] = [];
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
            repeat: wireRepeat(step.repeat),
            local: clock(step),
          });
          break;
        case 'complete': {
          const done = mock.handle('todo_task_complete', { id, local: clock(step) }) as Completion;
          if (done.occurrence) occurrences.push(done.occurrence.id);
          break;
        }
        case 'skip':
          mock.handle('todo_task_skip', { id, local: clock(step) });
          break;
        case 'reopen_occurrence':
          mock.handle('todo_task_reopen', { id: pick(step.occurrence, occurrences) });
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
        case 'priority':
          mock.handle('todo_task_set_priority', { id, priority: step.priority });
          break;
        case 'update':
          mock.handle('todo_task_update', {
            id,
            title: step.title,
            notes: step.notes,
            sessionId: step.session,
          });
          break;
        case 'comment_add':
          comments.push(
            (mock.handle('todo_comment_add', { taskId: id, body: step.body }) as Comment).id,
          );
          break;
        case 'comment_edit':
          mock.handle('todo_comment_edit', { id: pick(step.comment, comments), body: step.body });
          break;
        case 'comment_delete':
        case 'comment_restore':
          mock.handle(`todo_${step.op}`, { id: pick(step.comment, comments) });
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
        case 'pref':
          prefs.set(`todo.${step.key}`, step.value);
          break;
        case 'reorder': {
          const logged = mock.ops().length;
          const target = (step.target ?? []).map((k) =>
            k.kind === 'list' ? { kind: 'list', list_id: pick(k.list, lists) } : k,
          );
          try {
            mock.handle('todo_task_reorder', {
              id,
              before: pick(step.before, tasks),
              after: pick(step.after, tasks),
              target,
              local: clock(step),
            });
          } finally {
            reorderOps.push(mock.ops().slice(logged));
          }
          break;
        }
        case 'list_reorder':
          mock.handle('todo_list_reorder', {
            id: list,
            before: pick(step.before, lists),
            after: pick(step.after, lists),
          });
          break;
        case 'list_sort':
          mock.handle('todo_list_set_sort', { id: list, sort: step.sort });
          break;
        case 'list_group':
          mock.handle('todo_list_set_group', { id: list, groupBy: step.group_by });
          break;
        default:
          mock.handle(`todo_task_${step.op}`, { id });
      }
      results.push('ok');
    } catch (e) {
      results.push((e as AppError).code);
    }
  }
  const view = clock();
  const inbox = mock.handle('todo_inbox', { local: view }) as InboxView;
  const sidebar = mock.handle('todo_lists', { local: view }) as SidebarView;
  const all = mock.handle('todo_all', { local: view }) as AllView;
  const listNames = new Map(sidebar.lists.map((e) => [e.list.id, e.list.name]));
  const shape = (v: { groups: TaskGroup[] }) =>
    v.groups.map((g) => [groupLabel(g.key, listNames), g.tasks.map((t) => t.title)]);
  const views: Record<string, unknown> = {
    today: shape(mock.handle('todo_today', { local: view }) as TodayView),
    upcoming: shape(mock.handle('todo_upcoming', { local: view }) as UpcomingView),
    inbox: shape(inbox),
    all: shape(all),
  };
  lists.forEach((id, i) => {
    try {
      views[`list${i}`] = shape(mock.handle('todo_list', { id, local: view }) as ListView);
    } catch {
      // A deleted List has no page.
    }
  });
  const titles = (ts: Task[]) => ts.map((t) => t.title);
  const todayView = mock.handle('todo_today', { local: view }) as TodayView;
  const upcoming = mock.handle('todo_upcoming', { local: view }) as UpcomingView;
  const doneView = mock.handle('todo_done', { before: null, days: 60, local: view }) as DoneView;
  const counts = mock.handle('todo_day_counts', {
    from: view.today,
    to: '2027-12-31',
    exclude: null,
  }) as DayCount[];
  const commentRows = [
    ...tasks.map((taskId) => [taskId, ''] as const),
    ...occurrences.map((taskId) => [taskId, ' (occurrence)'] as const),
  ].flatMap(([taskId, suffix]) => {
    try {
      const task = mock.handle('todo_task', { id: taskId }) as Task;
      const list = mock.handle('todo_comments', { taskId }) as Comment[];
      return [[`${task.title}${suffix}`, list.map((c) => `${c.body}${c.edited ? '*' : ''}`)]];
    } catch {
      return [];
    }
  });
  let history: string[] = [];
  try {
    const lines = mock.handle('todo_task_history', {
      id: tasks[c.history_of ?? 0],
    }) as HistoryLine[];
    history = lines.map((l) => l.text);
  } catch {
    // A Task that is gone has no History.
  }
  return {
    results,
    comments: commentRows,
    history,
    wont: doneView.days.flatMap((d) =>
      d.tasks.filter((t) => t.state === 'wont').map((t) => t.title),
    ),
    priorities: all.groups.flatMap((g) => g.tasks.map((t) => [t.title, t.priority])),
    dues: all.groups.flatMap((g) => g.tasks.map((t) => [t.title, t.due_date, t.due_time])),
    today: { overdue: titles(overdueOf(todayView)), tasks: titles(tasksOf(todayView)) },
    upcoming: {
      overdue: titles(overdueOf(upcoming)),
      days: upcoming.groups
        .filter((g) => g.key.kind === 'day')
        .map((g) => [g.key.kind === 'day' ? g.key.date : '', titles(g.tasks)]),
    },
    views,
    reorder_ops: reorderOps,
    day_counts: counts.map((d) => [d.date, d.count]),
    inbox: inbox.groups
      .filter((g) => g.key.kind !== 'no_date_in_list')
      .flatMap((g) => g.tasks.map((t) => t.title)),
    lists: sidebar.lists.map((e) => e.list.name),
    emojis: sidebar.lists.map((e) => e.list.emoji),
    sidebar_today: [sidebar.today_count, sidebar.overdue_count],
    counts: [sidebar.inbox_count, ...sidebar.lists.map((e) => e.open_count)],
    all: all.groups.map((g) => [
      g.key.kind === 'list' && g.key.list_id ? (listNames.get(g.key.list_id) ?? '?') : 'Inbox',
      g.tasks.map((t) => t.title),
    ]),
    repeats: all.groups.flatMap((g) =>
      g.tasks
        .filter((t) => t.repeat_freq !== null)
        .map((t) => [t.title, t.repeat_freq, t.repeat_every, t.repeat_days, t.repeat_mode]),
    ),
    done: doneView.days.flatMap((d) => d.tasks.map((t) => t.title)),
  };
}

describe('the Todo mock follows the shared rule fixtures', () => {
  for (const c of [...rules.cases, ...orderRules.cases] as Case[]) {
    it(`${c.rule}: ${c.name}`, () => {
      const actual = run(c);
      const picked = Object.fromEntries(
        Object.keys(c.expect).map((k) => [
          k,
          // `views` holds one entry per view: a case names only the ones it checks.
          k === 'views'
            ? Object.fromEntries(
                Object.keys(c.expect.views as object).map((v) => [v, actual.views[v]]),
              )
            : actual[k as keyof typeof actual],
        ]),
      );
      expect(picked).toEqual(c.expect);
    });
  }
});

describe('the Todo mock: Done', () => {
  const kolkata: LocalNow = { today: '2026-10-21', now: '14:05', offset_min: 330 };

  /** Five Tasks done at fixed UTC instants on four device days (Kolkata). */
  function doneAt() {
    vi.useFakeTimers();
    const mock = createTodoMock();
    const stamps: [string, string][] = [
      ['a', '2026-10-21T10:00:00Z'],
      ['b', '2026-10-21T12:00:00Z'],
      ['c', '2026-10-20T08:00:00Z'],
      ['d', '2026-10-18T20:00:00Z'],
      ['e', '2026-10-15T04:00:00Z'],
    ];
    for (const [title, at] of stamps) {
      const { id } = mock.handle('todo_task_create', { title }) as Task;
      vi.setSystemTime(new Date(at));
      mock.handle('todo_task_complete', { id, local: LOCAL });
    }
    return mock;
  }
  const page = (mock: ReturnType<typeof createTodoMock>, before: string | null, days: number) =>
    mock.handle('todo_done', { before, days, local: kolkata }) as DoneView;
  const shape = (v: DoneView) => v.days.map((d) => [d.date, d.tasks.map((t) => t.title)]);

  it('pages by day, newest first, with a cursor', () => {
    try {
      const mock = doneAt();
      const first = page(mock, null, 2);
      expect(shape(first)).toEqual([
        ['2026-10-21', ['b', 'a']],
        ['2026-10-20', ['c']],
      ]);
      expect(first.next_cursor).toBe('2026-10-20');
      const second = page(mock, first.next_cursor, 2);
      expect(shape(second)).toEqual([
        ['2026-10-19', ['d']],
        ['2026-10-15', ['e']],
      ]);
      expect(second.next_cursor).toBeNull();
      expect(page(mock, '2026-10-15', 2).days).toEqual([]);
      expect(() => page(mock, 'nope', 2)).toThrow();
    } finally {
      vi.useRealTimers();
    }
  });

  it('unticks Checklist items the way Rust does', () => {
    expect(
      resetChecklist('Buy:\n- [x] milk\n- [ ] eggs\n  * [X] nested\n+ [x]\nplain - [x] text'),
    ).toBe('Buy:\n- [ ] milk\n- [ ] eggs\n  * [ ] nested\n+ [ ]\nplain - [x] text');
  });
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
      mock.handle('todo_task_complete', { id: a.id, local });
      vi.setSystemTime(new Date('2026-10-20T18:29:59Z')); // a minute before Kolkata's midnight
      mock.handle('todo_task_complete', { id: b.id, local });
      const view = mock.handle('todo_today', { local }) as TodayView;
      expect(view.done.map((t) => t.title)).toEqual(['a']);
      expect(tasksOf(view).map((t) => t.title)).toEqual(['c']);
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
    const view = mock.handle('todo_inbox', { local: LOCAL }) as InboxView;
    expect(view.groups[0]?.tasks[0]?.title).toBe('a');
  });

  it('sets and clears the done time', () => {
    const mock = createTodoMock();
    const { id } = mock.handle('todo_task_create', { title: 'a' }) as Task;
    const { task: done, occurrence } = mock.handle('todo_task_complete', {
      id,
      local: LOCAL,
    }) as Completion;
    expect(done.state).toBe('done');
    expect(occurrence).toBeNull();
    expect(done.closed_at).toBeTypeOf('number');
    const back = mock.handle('todo_task_reopen', { id }) as Task;
    expect([back.state, back.closed_at]).toEqual(['open', null]);
  });

  it('counts characters, not UTF-16 units, for the length limit', () => {
    expect(validateTitle('😀'.repeat(500))).toHaveLength(1000);
    expect(() => validateTitle('😀'.repeat(501))).toThrow();
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
    mock.handle('todo_task_complete', { id: a.id, local: LOCAL });
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

// Capture: the same cases as src-tauri/src/todo_capture_tests.rs, against the mock.
describe('capture in the browser mock', () => {
  const clock = { today: '2026-10-21', now: '14:05', offset_min: 0 };
  const monthly = { freq: 'month', every: 1, days: null, mode: 'fixed' };
  const parse = (mock: ReturnType<typeof createTodoMock>, text: string) =>
    mock.handle('todo_parse', {
      request: {
        text,
        ignored: [],
        prefer: {},
        frozen: [],
        today: '2026-10-21',
        now: '14:05',
        week_start: 'mon',
      },
    }) as import('./bindings').Parsed;
  const editOf = (t: Task) => ({
    title: t.title,
    understood: t.understood,
    due_date: t.due_date,
    due_time: t.due_time,
    repeat: null,
    priority: t.priority,
    list_id: t.list_id,
  });
  const quick = (mock: ReturnType<typeof createTodoMock>, t: Task, edit: unknown) =>
    mock.handle('todo_task_quick_edit', { id: t.id, edit, local: clock }) as Task;
  const code = (fn: () => unknown) => {
    try {
      fn();
    } catch (e) {
      return (e as AppError).code;
    }
    return null;
  };

  it('parses #Name against the live Lists, and offers a new one without making it', () => {
    const mock = createTodoMock();
    const home = mock.handle('todo_list_create', { name: 'Home' }) as TodoList;
    expect(parse(mock, 'Fix tap #ho tomorrow')).toMatchObject({
      list_id: home.id,
      date: '2026-10-22',
      new_list: null,
    });
    expect(parse(mock, 'Buy paint #Garage').new_list).toBe('Garage');
    const sidebar = mock.handle('todo_lists', { local: clock }) as SidebarView;
    expect(sidebar.lists).toHaveLength(1);
  });

  it('creates a Task with notes, Priority, Repeat and the understood record', () => {
    const mock = createTodoMock();
    const understood = '[{"word":"tomorrow","kind":"date","value":"2026-10-22"}]';
    const task = mock.handle('todo_task_create', {
      title: 'Pay rent tomorrow',
      when: { date: '2026-10-22', time: '09:00', local: clock },
      details: { notes: '- [ ] transfer', priority: 1, repeat: monthly, understood },
    }) as Task;
    expect(task).toMatchObject({
      notes: '- [ ] transfer',
      priority: 1,
      due_date: '2026-10-22',
      due_time: '09:00',
      repeat_freq: 'month',
      repeat_days: 22,
      understood,
    });
  });

  it('gives a Repeat with no date its first day, and refuses one with no clock', () => {
    const mock = createTodoMock();
    const task = mock.handle('todo_task_create', {
      title: 'Gym',
      when: { date: null, time: null, local: clock },
      details: { repeat: { freq: 'week', every: 1, days: 18, mode: 'fixed' } },
    }) as Task;
    expect(task.due_date).toBe('2026-10-22');
    expect(
      code(() => mock.handle('todo_task_create', { title: 'Gym', details: { repeat: monthly } })),
    ).toBe('validation');
  });

  it('validates the details', () => {
    const mock = createTodoMock();
    for (const details of [
      { priority: 5 },
      { understood: 'not json' },
      { understood: '{"a":1}' },
    ]) {
      expect(code(() => mock.handle('todo_task_create', { title: 'x', details }))).toBe(
        'validation',
      );
    }
    expect(
      (mock.handle('todo_inbox', { local: LOCAL }) as InboxView).groups[0]?.tasks,
    ).toHaveLength(0);
  });

  it('saves a Quick edit in one update and does nothing when nothing changed', () => {
    const mock = createTodoMock();
    const work = mock.handle('todo_list_create', { name: 'Work' }) as TodoList;
    const task = mock.handle('todo_task_create', { title: 'Pay rent' }) as Task;
    const same = quick(mock, task, editOf(task));
    expect(same.updated_at).toBe(task.updated_at);
    const saved = quick(mock, task, {
      ...editOf(task),
      title: 'Pay rent tomorrow p1 #work',
      due_date: '2026-10-22',
      priority: 1,
      list_id: work.id,
    });
    expect(saved).toMatchObject({
      title: 'Pay rent tomorrow p1 #work',
      due_date: '2026-10-22',
      priority: 1,
      list_id: work.id,
    });
  });

  it('refuses a past date in a Quick edit, but Undo may put one back', () => {
    const mock = createTodoMock();
    const task = mock.handle('todo_task_create', { title: 'Old' }) as Task;
    const past = { ...editOf(task), due_date: '2026-10-20' };
    expect(code(() => quick(mock, task, past))).toBe('validation');
    const undone = mock.handle('todo_task_quick_edit_undo', {
      id: task.id,
      edit: past,
      local: clock,
    }) as Task;
    expect(undone.due_date).toBe('2026-10-20');
    // An edit that leaves the date alone keeps the Overdue one.
    const kept = quick(mock, undone, { ...editOf(undone), title: 'Old, renamed' });
    expect([kept.title, kept.due_date]).toEqual(['Old, renamed', '2026-10-20']);
  });

  it('validates a Quick edit and needs an open Task for anything but the title', () => {
    const mock = createTodoMock();
    const task = mock.handle('todo_task_create', { title: 'Pay rent' }) as Task;
    for (const bad of [
      { title: '  ' },
      { title: 'two\nlines' },
      { priority: 7 },
      { list_id: 'nope' },
      { understood: 'nope' },
    ]) {
      expect(code(() => quick(mock, task, { ...editOf(task), ...bad }))).not.toBeNull();
    }
    mock.handle('todo_task_complete', { id: task.id, local: LOCAL });
    expect(code(() => quick(mock, task, { ...editOf(task), priority: 1 }))).toBe('validation');
    const done = mock.handle('todo_task', { id: task.id }) as Task;
    expect(quick(mock, done, { ...editOf(done), title: 'Paid rent' }).title).toBe('Paid rent');
  });

  it('keeps a Repeat when a Quick edit moves its date', () => {
    const mock = createTodoMock();
    const task = mock.handle('todo_task_create', {
      title: 'Pay rent',
      when: { date: '2026-10-25', time: null, local: clock },
      details: { repeat: monthly },
    }) as Task;
    const saved = quick(mock, task, {
      ...editOf(task),
      repeat: { ...monthly, days: 25 },
      due_date: '2026-10-28',
    });
    expect([saved.due_date, saved.repeat_freq, saved.repeat_days]).toEqual([
      '2026-10-28',
      'month',
      28,
    ]);
  });
});
