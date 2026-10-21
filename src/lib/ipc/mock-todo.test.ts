/**
 * Runs the shared rule fixtures (fixtures/todo-rules.json) against the browser mock. The Rust
 * commands run the same file in src-tauri/src/todo.rs, so the mock cannot drift from them
 * (ADR 0009). Keep `run` in step with `run_rule_case` there.
 */
import { describe, expect, it } from 'vitest';
import type { AppError, Task } from './bindings';
import rules from './fixtures/todo-rules.json';
import { createTodoMock, keyAfter, validateTitle } from './mock-todo';

interface Step {
  op: 'create' | 'complete' | 'reopen';
  title?: string;
  task?: number | string;
}
interface Case {
  rule: string;
  name: string;
  steps: Step[];
  expect: Record<string, unknown>;
}

function run(c: Case) {
  const mock = createTodoMock();
  const created: string[] = [];
  const results: string[] = [];
  for (const step of c.steps) {
    const id = typeof step.task === 'number' ? created[step.task] : step.task;
    try {
      const out = mock.handle(`todo_task_${step.op}`, { title: step.title, id }) as Task;
      if (step.op === 'create') created.push(out.id);
      results.push('ok');
    } catch (e) {
      results.push((e as AppError).code);
    }
  }
  const view = mock.handle('todo_inbox', {}) as { tasks: Task[] };
  return { results, inbox: view.tasks.map((t) => t.title) };
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
});
