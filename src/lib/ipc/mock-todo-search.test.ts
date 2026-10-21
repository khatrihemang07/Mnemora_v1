/**
 * Runs the shared search fixtures (fixtures/todo-search-rules.json) against the browser mock. The
 * Rust command runs the same file in src-tauri/src/todo_search_tests.rs (ADR 0009). Keep `build`
 * in step with `build` there.
 */
import { describe, expect, it } from 'vitest';
import type { Comment, LocalNow, Span, Task, TaskHit, TaskSearchResults } from './bindings';
import rules from './fixtures/todo-search-rules.json';
import { createTodoMock } from './mock-todo';

interface FixtureTask {
  title: string;
  notes?: string;
  comments?: string[];
  deleted_comments?: string[];
  state?: 'open' | 'done' | 'wont' | 'deleted';
}

const LOCAL: LocalNow = { today: '2026-10-21', now: '14:05', offset_min: 0 };

function build(tasks: FixtureTask[]) {
  const mock = createTodoMock();
  const call = <T>(cmd: string, args: Record<string, unknown>) => mock.handle(cmd, args) as T;
  for (const t of tasks) {
    const task = call<Task>('todo_task_create', { title: t.title });
    if (t.notes !== undefined) {
      call('todo_task_update', { id: task.id, title: t.title, notes: t.notes, sessionId: 's' });
    }
    for (const body of t.comments ?? []) call('todo_comment_add', { taskId: task.id, body });
    for (const body of t.deleted_comments ?? []) {
      const c = call<Comment>('todo_comment_add', { taskId: task.id, body });
      call('todo_comment_delete', { id: c.id });
    }
    if (t.state === 'done') call('todo_task_complete', { id: task.id, local: LOCAL });
    if (t.state === 'wont') call('todo_task_wont_do', { id: task.id });
    if (t.state === 'deleted') call('todo_task_delete', { id: task.id });
  }
  return (query: string) => call<TaskSearchResults>('todo_search', { query });
}

const titles = (hits: TaskHit[]) => hits.map((h) => h.task.title);
const flat = (spans: Span[]) => ({
  text: spans.map((s) => s.text).join(''),
  marked: spans.filter((s) => s.matched).map((s) => s.text),
});

describe('the shared search fixtures', () => {
  for (const c of rules.cases) {
    it(c.name, () => {
      const search = build(c.tasks as FixtureTask[]);
      for (const q of c.queries) {
        const found = search(q.q);
        expect(titles(found.exact), `exact for ${q.q}`).toEqual(q.exact);
        expect(titles(found.similar), `similar for ${q.q}`).toEqual(q.similar);
      }
      for (const [title, m] of Object.entries((c as { marked?: Record<string, { q: string; words: string[] }> }).marked ?? {})) {
        const hit = search(m.q).exact.find((h) => h.task.title === title);
        expect(flat(hit?.title ?? []).marked).toEqual(m.words);
      }
      const snippets = (c as { snippets?: Record<string, { q: string; source: string | null; marked: string[]; text: string }> }).snippets ?? {};
      for (const [title, s] of Object.entries(snippets)) {
        const hit = search(s.q).exact.find((h) => h.task.title === title);
        expect(hit?.snippet_source).toBe(s.source);
        expect(flat(hit?.snippet ?? [])).toEqual({ text: s.text, marked: s.marked });
      }
    });
  }

  it('hostile input is never an error', () => {
    const search = build([{ title: 'Anything' }]);
    for (const q of ['"', '*', 'NEAR(', 'a:b', 'x OR', '(', '"""', '-']) {
      expect(search(q).exact).toBeInstanceOf(Array);
    }
  });
});
