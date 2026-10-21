/**
 * Runs the shared rule fixtures (fixtures/todo-rules.json) against the browser mock. The Rust
 * commands run the same file in src-tauri/src/todo.rs, so the mock cannot drift from them
 * (ADR 0009). Keep `run` in step with `run_rule_case` there.
 */
import { describe, expect, it } from 'vitest';
import type { AllView, AppError, ListDeletion, SidebarView, Task, TodoList } from './bindings';
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
    | 'list_restore';
  title?: string;
  name?: string;
  emoji?: string;
  task?: number | string;
  list?: number | string | null;
  choice?: 'delete' | 'inbox' | { list: number | string };
  deletion?: number;
}
interface Case {
  rule: string;
  name: string;
  steps: Step[];
  expect: Record<string, unknown>;
}

/** A number picks the n-th created item, null stays null, a string is used as an id. */
const pick = <T>(v: number | string | null | undefined, made: T[]) =>
  typeof v === 'number' ? made[v] : (v ?? null);

function run(c: Case) {
  const mock = createTodoMock();
  const tasks: string[] = [];
  const lists: string[] = [];
  const deletions: ListDeletion[] = [];
  const results: string[] = [];
  for (const step of c.steps) {
    const id = pick(step.task, tasks);
    const list = pick(step.list, lists);
    try {
      switch (step.op) {
        case 'create':
          tasks.push(
            (mock.handle('todo_task_create', { title: step.title, listId: list }) as Task).id,
          );
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
  const sidebar = mock.handle('todo_lists', {}) as SidebarView;
  const all = mock.handle('todo_all', {}) as AllView;
  return {
    results,
    inbox: inbox.tasks.map((t) => t.title),
    lists: sidebar.lists.map((e) => e.list.name),
    emojis: sidebar.lists.map((e) => e.list.emoji),
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
    const entry = (mock.handle('todo_lists', {}) as SidebarView).lists[0];
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
