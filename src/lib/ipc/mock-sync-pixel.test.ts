import { describe, expect, it, vi } from 'vitest';
import type { ConflictList, Task } from './bindings';
import { createJournalMock } from './mock-journal';
import { createActivityMock } from './mock-sync-activity';
import { createConflictsMock } from './mock-sync-conflicts';
import { breakdown, createPixelMock } from './mock-sync-pixel';
import type { Formats } from './mock-sync-words';
import { createTodoMock } from './mock-todo';

function setup(formats?: () => Formats) {
  const todo = createTodoMock();
  const journal = createJournalMock();
  // The Conflicts mock logs to the Activity mock, as mock.ts wires them.
  const conflicts = createConflictsMock(
    undefined,
    Date.now,
    () => true,
    (line) => activity.add(line),
  );
  const activity = createActivityMock(0, Date.now(), (id) => conflicts.isOpen(id));
  const announce = {
    todoChanged: vi.fn(),
    journalChanged: vi.fn(),
    listsMerged: vi.fn(),
    taskDeleted: vi.fn(),
    entryDeleted: vi.fn(),
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
    formats,
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

  it('tells the open Journal about the Entry a scenario staged', () => {
    const t = setup();
    t.pixel.scenario('conflict');
    expect(t.announce.journalChanged).toHaveBeenCalled();
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

  it('settles a Conflict without asking when its Entry is deleted afterwards (R5)', () => {
    const t = setup();
    t.pixel.scenario('conflict');
    const entry = t.lists().needs_you.find((c) => c.module === 'journal');
    expect(entry).toBeDefined();
    const waiting = t.conflicts.openCount();
    const entryId = (t.pixel.world().find((d) => d.entity === 'entry') as { id: string }).id;
    t.journal.handle('journal_entry_delete', { id: entryId });
    expect(t.lists().needs_you.map((c) => c.module)).toEqual(['todo']);
    const view = t.conflicts.handle('sync_conflict', { id: entry?.id }) as { open: boolean };
    expect(view.open).toBe(false);
    expect(t.conflicts.openCount()).toBe(waiting - 1);
    const row = t.lists().resolved.find((r) => r.kind === 'conflict_deleted');
    expect(row?.detail).toMatch(/^Deleted, so nothing was asked · .* and .* both rewrote it$/);
    // Once, however often it is listed.
    t.lists();
    expect(t.lists().resolved.filter((r) => r.kind === 'conflict_deleted')).toHaveLength(1);
  });

  it('logs one Activity line when a Conflict closes because its record was deleted (R5-activity)', () => {
    const t = setup();
    t.pixel.scenario('conflict');
    const lines = () =>
      (
        t.activity.handle('sync_activity', {
          filter: { kind: 'all' },
          cursor: null,
          limit: 200,
        }) as {
          rows: { id: string; kind: string; text: { text: string }[] }[];
        }
      ).rows.filter((r) => r.kind === 'conflict_resolved');
    const before = lines().length;
    const entryId = (t.pixel.world().find((d) => d.entity === 'entry') as { id: string }).id;
    t.journal.handle('journal_entry_delete', { id: entryId });
    t.lists();
    t.lists();
    const after = lines();
    expect(after).toHaveLength(before + 1);
    expect(after.some((r) => r.id.startsWith('mock-act-resolved-'))).toBe(true);
    expect(
      after
        .find((r) => r.id.startsWith('mock-act-resolved-'))
        ?.text.map((p) => p.text)
        .join(''),
    ).toMatch(/^Conflict in .* resolved$/);
  });

  it('takes the Review link off a Conflict’s Activity line once it is resolved', () => {
    const t = setup();
    t.pixel.scenario('conflict');
    const rows = () =>
      (
        t.activity.handle('sync_activity', {
          filter: { kind: 'all' },
          cursor: null,
          limit: 50,
        }) as {
          rows: { kind: string; link: string | null }[];
        }
      ).rows.filter((r) => r.kind === 'conflict_raised');
    expect(rows().map((r) => r.link)).toEqual(['conflicts', 'conflicts']);
    for (const c of t.lists().needs_you)
      t.conflicts.handle('sync_conflict_resolve', { id: c.id, choice: { kind: 'keep_this' } });
    expect(rows().map((r) => r.link)).toEqual([null, null]);
  });

  it('puts a Priority clash, a delete that met an edit, a Task sent to the Inbox and the folded order line in Resolved automatically', () => {
    const t = setup();
    t.pixel.scenario('resolved');
    expect(
      t
        .lists()
        .resolved.map((r) => r.kind)
        .sort(),
    ).toEqual(['delete_edit', 'order_prefs', 'task_inbox', 'value']);
    const value = t.lists().resolved.find((r) => r.kind === 'value');
    expect(value?.title).toBe('‘Water plants’ · Priority');
    expect(value?.detail).toBe('P2 (Mac) kept · P1 (Pixel 8 (simulated)) replaced');
    // Like Rust: the edited units in words, never a bare "edited (" (R6).
    const gone = t.lists().resolved.find((r) => r.kind === 'delete_edit');
    expect(gone?.detail).toMatch(/^Deleted \(.+\) kept · [a-z, ]+ edited \(.+\) kept inside it$/);
  });

  it('"Use P1 instead" writes the other value as a real edit, once (M2)', () => {
    const t = setup();
    t.pixel.scenario('resolved');
    const row = t.lists().resolved.find((r) => r.kind === 'value');
    expect(row?.action).toBe('Use P1 instead');
    const water = t.todo
      .exportDocs()
      .find((d) => d.entity === 'task' && d.values.title === 'Water plants') as { id: string };
    expect((t.todo.handle('todo_task', { id: water.id }) as Task).priority).toBe(2);
    t.announce.todoChanged.mockClear();
    t.conflicts.handle('sync_resolution_use_other', { id: row?.id });
    expect((t.todo.handle('todo_task', { id: water.id }) as Task).priority).toBe(1);
    expect(t.announce.todoChanged).toHaveBeenCalled();
    expect(t.lists().resolved.find((r) => r.id === row?.id)).toMatchObject({
      action: null,
      used: 'Changed to P1',
    });
    const history = t.todo.handle('todo_task_history', { id: water.id }) as unknown[];
    expect(JSON.stringify(history)).toContain('priority');
    expect(() => t.conflicts.handle('sync_resolution_use_other', { id: row?.id })).toThrow();
    // …and the change reaches the phone on the next sync.
    t.pixel.sync();
    expect(t.pixel.world().find((d) => d.id === water.id)?.values.priority).toBe(1);
  });

  it('writes dates and times in the Preferences’ format, not as raw ISO (M3)', () => {
    const rows = (formats: Formats) => {
      const t = setup(() => formats);
      const task = t.todo.handle('todo_task_create', {
        title: 'Call bank',
        listId: null,
        when: {
          date: '2026-11-01',
          time: '18:00',
          local: { today: '2026-10-22', now: '10:00', offset_min: 0 },
        },
      }) as Task;
      t.pixel.sync();
      t.pixel.edit('task_when', { task: task.id, date: '2026-11-05', time: '09:00' });
      t.todo.handle('todo_tasks_restore_when', {
        changes: [{ id: task.id, due_date: '2026-11-07', due_time: '21:30', repeat: null }],
      });
      t.pixel.sync();
      return t.lists().resolved.find((r) => r.title.endsWith('Due date'));
    };
    expect(rows({ date: 'long', time: '24h' })).toMatchObject({
      title: '‘Call bank’ · Due date',
      detail: 'Sat 7 Nov 21:30 (Mac) kept · Thu 5 Nov 09:00 (Pixel 8 (simulated)) replaced',
      action: 'Use Thu 5 Nov 09:00 instead',
    });
    expect(rows({ date: 'short', time: '12h' })?.detail).toBe(
      'Nov 7, 2026 9:30 PM (Mac) kept · Nov 5, 2026 9:00 AM (Pixel 8 (simulated)) replaced',
    );
    expect(rows({ date: 'iso', time: '24h' })?.action).toBe('Use 2026-11-05 09:00 instead');
  });

  it('keeps Repeat and the understood words on a Task the phone synced (M1)', () => {
    const t = setup();
    const task = t.todo.handle('todo_task_create', {
      title: 'Water plants',
      listId: null,
      when: {
        date: '2026-10-22',
        time: null,
        local: { today: '2026-10-22', now: '10:00', offset_min: 0 },
      },
      details: {
        repeat: { freq: 'week', every: 1, days: null, mode: 'fixed' },
        understood: '["weekly"]',
      },
    }) as Task;
    t.pixel.sync();
    t.pixel.edit('task_edit', { task: task.id, title: 'Water the plants' });
    t.pixel.sync();
    const back = t.todo.exportDocs().find((d) => d.id === task.id);
    expect(back?.values).toMatchObject({
      title: 'Water the plants',
      repeat_freq: 'week',
      repeat_every: 1,
      repeat_mode: 'fixed',
      understood: '["weekly"]',
    });
    expect(t.pixel.world().find((d) => d.id === task.id)?.values.repeat_freq).toBe('week');
  });

  it('folds the order keys and Preferences a sync took into one line (M5)', () => {
    const t = setup();
    t.todo.handle('todo_task_create', { title: 'Call mum', listId: null });
    t.todo.handle('todo_task_create', { title: 'Buy oat milk', listId: null });
    t.pixel.sync();
    t.pixel.edit('task_place', { task: 'Buy oat milk', before: 'Call mum' });
    t.pixel.edit('pref', { key: 'core.week_start', value: 'sun' });
    t.pixel.edit('pref', { key: 'core.time_format', value: '12h' });
    t.pixel.sync();
    const fold = t.lists().resolved.find((r) => r.kind === 'order_prefs');
    expect(fold).toMatchObject({
      title: 'Order & preferences · 3 changes',
      detail: 'Pixel 8 (simulated)',
      module: 'sync',
      icon: 'order',
      action: null,
    });
    expect(fold?.changes).toEqual(['‘Buy oat milk’ moved', 'Week start', 'Time format']);
    // Nothing new, no new line.
    const before = t.lists().resolved.length;
    t.pixel.sync();
    expect(t.lists().resolved).toHaveLength(before);
  });

  it('merges Lists with one name and tells the follower, once the person has had time to open one', () => {
    const t = setup();
    t.pixel.scenario('listMerged');
    expect(t.announce.listsMerged).not.toHaveBeenCalled();
    t.pixel.sync();
    expect(t.announce.listsMerged).toHaveBeenCalledOnce();
    const groceries = t.todo
      .exportDocs()
      .filter((d) => d.entity === 'list' && d.values.name === 'Groceries');
    const kept = groceries.filter((d) => d.values.deleted_at == null);
    expect(kept).toHaveLength(1);
    // The one made here is the one that went away, and its Task moved to the kept List.
    // The phone's List is the kept one, so the List you are on is the one that goes away.
    expect(kept[0]?.id).toMatch(/^-pixel-list-/);
    expect(t.announce.listsMerged.mock.calls[0]?.[0]).toMatchObject({
      from_id: groceries.find((d) => d.values.deleted_at != null)?.id,
      into_id: kept[0]?.id,
    });
    const oat = t.todo
      .exportDocs()
      .find((d) => d.entity === 'task' && d.values.title === 'Oat milk');
    expect(oat?.values.list_id).toBe(kept[0]?.id);
    expect(t.announce.listsMerged.mock.calls[0]?.[0]).toMatchObject({
      from_name: 'Groceries',
      into_name: 'Groceries',
      device: 'Pixel 8 (simulated)',
    });
  });

  it('updates "Call mum" in place every time the remote scenario runs', () => {
    const t = setup();
    t.pixel.scenario('remote');
    expect(t.titles()).toEqual(['Call mum at 6']);
    t.pixel.scenario('remote');
    expect(t.titles()).toEqual(['Call mum at 6']);
    expect(t.lists().needs_you).toEqual([]);
  });

  it('deletes a Task here when the phone deleted it', () => {
    const t = setup();
    t.pixel.scenario('remoteDelete');
    expect(t.titles()).toEqual(['Call mum']);
    t.pixel.sync();
    expect(t.titles()).toEqual([]);
    expect(t.announce.taskDeleted).toHaveBeenCalledOnce();
    expect(t.announce.taskDeleted.mock.calls[0]?.[0]).toMatchObject({
      title: 'Call mum',
      device: 'Pixel 8 (simulated)',
    });
    t.pixel.sync();
    expect(t.announce.taskDeleted).toHaveBeenCalledOnce();
  });

  it('tells the editor when the phone deleted an Entry that was showing, once', () => {
    const t = setup();
    t.pixel.scenario('remoteEntryDelete');
    expect(t.announce.entryDeleted).not.toHaveBeenCalled();
    t.pixel.sync();
    expect(t.announce.entryDeleted).toHaveBeenCalledOnce();
    expect(t.announce.entryDeleted.mock.calls[0]?.[0]).toMatchObject({
      device: 'Pixel 8 (simulated)',
    });
    t.pixel.sync();
    expect(t.announce.entryDeleted).toHaveBeenCalledOnce();
  });

  it('words a session’s breakdown as Rust does', () => {
    expect(breakdown(['task', 'task', 'list', 'entry', 'setting'])).toBe(
      'Todo: 2 Tasks, 1 List · Journal: 1 Entry · Settings: 1',
    );
  });
});
