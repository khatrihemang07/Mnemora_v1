import { describe, expect, it, vi } from 'vitest';
import type { ConflictList, Task } from './bindings';
import { createJournalMock } from './mock-journal';
import { createActivityMock } from './mock-sync-activity';
import { createConflictsMock } from './mock-sync-conflicts';
import { breakdown, createPixelMock } from './mock-sync-pixel';
import { createTodoMock } from './mock-todo';

function setup() {
  const todo = createTodoMock();
  const journal = createJournalMock();
  const conflicts = createConflictsMock();
  const activity = createActivityMock(0);
  const announce = {
    todoChanged: vi.fn(),
    journalChanged: vi.fn(),
    listsMerged: vi.fn(),
    activityChanged: vi.fn(),
    settingsChanged: vi.fn(),
    newConflicts: vi.fn(),
  };
  const prefs: Record<string, unknown> = {};
  const pixel = createPixelMock({
    data: { todo, journal },
    settings: {
      exportDocs: () =>
        Object.entries(prefs).map(([id, value]) => ({
          entity: 'setting' as const,
          id,
          values: { value: value as never },
        })),
      importDocs: (docs) => {
        for (const d of docs) prefs[d.id] = d.values.value;
        return docs.map((d) => d.id);
      },
    },
    command: (cmd, a) => (todo.handles(cmd) ? todo.handle(cmd, a) : journal.handle(cmd, a)),
    conflicts,
    activity,
    myName: () => 'Mac',
    announce,
  });
  // The mock also keeps its sample rows; these are the ones a sync made.
  const lists = () => {
    const all = conflicts.handle('sync_conflicts', {}) as ConflictList;
    return {
      needs_you: all.needs_you.filter((c) => c.id.startsWith('pixel-')),
      resolved: all.resolved.filter((r) => r.id.startsWith('pixel-')),
    };
  };
  const titles = () =>
    (
      todo.handle('todo_inbox', {
        local: { today: '2026-10-22', now: '10:00', offset_min: 0 },
      }) as { groups: { tasks: Task[] }[] }
    ).groups.flatMap((g) => g.tasks.map((t) => t.title));
  return { todo, journal, conflicts, activity, announce, prefs, pixel, lists, titles };
}

describe('the simulated Pixel 8', () => {
  it('syncs a change it made into this device’s data and tells the views', () => {
    const t = setup();
    t.todo.handle('todo_task_create', { title: 'Call mum', listId: null });
    t.pixel.sync();
    t.pixel.edit('task_edit', { task: 'Call mum', title: 'Call mum at 6' });
    const found = t.pixel.sync();
    expect(found).toMatchObject({ in: 1, out: 0, conflicts: 0 });
    expect(t.titles()).toEqual(['Call mum at 6']);
    expect(t.announce.todoChanged).toHaveBeenCalled();
    expect(t.announce.activityChanged).toHaveBeenCalled();
  });

  it('sends this device’s edits to the phone and merges edits to different words', () => {
    const t = setup();
    const made = t.todo.handle('todo_task_create', { title: 'Hike', listId: null }) as Task;
    t.todo.handle('todo_task_update', {
      id: made.id,
      title: 'Hike',
      notes: 'Call Sam on Saturday morning',
      sessionId: 'a',
    });
    t.pixel.sync();
    t.pixel.edit('task_edit', { task: 'Hike', notes: 'Call Sam on Sunday morning' });
    t.todo.handle('todo_task_update', {
      id: made.id,
      title: 'Hike',
      notes: 'Phone Sam on Saturday morning',
      sessionId: 'b',
    });
    t.pixel.sync();
    const task = t.todo.handle('todo_task', { id: made.id }) as Task;
    expect(task.notes).toBe('Phone Sam on Sunday morning');
    expect(t.lists().needs_you).toEqual([]);
  });

  it('raises Conflicts for the same words and writes the chosen text back', () => {
    const t = setup();
    const { result } = t.pixel.scenario('conflict');
    expect(result?.conflicts).toBe(2);
    const open = t.lists().needs_you;
    expect(open.map((c) => c.module).sort()).toEqual(['journal', 'todo']);
    const task = open.find((c) => c.module === 'todo');
    expect(task?.showing.device).toBe('Mac');
    expect(task?.other.device).toBe('Pixel 8 (simulated)');
    t.conflicts.handle('sync_conflict_resolve', {
      id: task?.id,
      choice: { kind: 'keep_other' },
    });
    const hike = t.todo.handle('todo_task', {
      id: (t.pixel.world().find((d) => d.values.title === 'Plan weekend hike') as { id: string })
        .id,
    }) as Task;
    expect(hike.notes).toContain('lake path');
    // A second sync finds the same clash again and does not list it twice.
    t.pixel.sync();
    expect(t.lists().needs_you).toHaveLength(1);
  });

  it('puts a Priority clash, a delete that met an edit and a Task sent to the Inbox in Resolved automatically', () => {
    const t = setup();
    t.pixel.scenario('resolved');
    expect(
      t
        .lists()
        .resolved.map((r) => r.kind)
        .sort(),
    ).toEqual(['delete_edit', 'task_inbox', 'value']);
    const value = t.lists().resolved.find((r) => r.kind === 'value');
    expect(value?.title).toBe('‘Water plants’ · Priority');
    expect(value?.detail).toBe('P2 (Mac) kept · P1 (Pixel 8 (simulated)) replaced');
  });

  it('merges Lists with one name and tells the follower, once the person has had time to open one', () => {
    const t = setup();
    t.pixel.scenario('listMerged');
    expect(t.announce.listsMerged).not.toHaveBeenCalled();
    t.pixel.sync();
    expect(t.announce.listsMerged).toHaveBeenCalledOnce();
    expect(t.announce.listsMerged.mock.calls[0]?.[0]).toMatchObject({
      from_name: 'Groceries',
      into_name: 'Groceries',
      device: 'Pixel 8 (simulated)',
    });
  });

  it('deletes a Task here when the phone deleted it', () => {
    const t = setup();
    t.pixel.scenario('remoteDelete');
    expect(t.titles()).toEqual(['Call mum']);
    t.pixel.sync();
    expect(t.titles()).toEqual([]);
  });

  it('words a session’s breakdown as Rust does', () => {
    expect(breakdown(['task', 'task', 'list', 'entry', 'setting'])).toBe(
      'Todo: 2 Tasks, 1 List · Journal: 1 Entry · Settings: 1',
    );
  });
});
