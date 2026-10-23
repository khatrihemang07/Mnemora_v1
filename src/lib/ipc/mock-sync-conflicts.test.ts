import { describe, expect, it, vi } from 'vitest';
import type { ConflictRow } from './bindings';
import { type Event, describe as lineOf } from './mock-sync-activity';
import { createConflictsMock, diffSpans } from './mock-sync-conflicts';

describe('conflicts mock', () => {
  it('marks the words each text has that the other lacks', () => {
    const [a, b] = diffSpans('run, felt great.', 'run, legs heavy.');
    expect(a.filter((s) => s.changed).map((s) => s.text)).toEqual(['felt great.']);
    expect(b.filter((s) => s.changed).map((s) => s.text)).toEqual(['legs heavy.']);
  });

  it('lists, drafts, resolves and then no longer lists', () => {
    const mock = createConflictsMock();
    const list = mock.handle('sync_conflicts', {}) as { needs_you: { id: string }[] };
    expect(list.needs_you).toHaveLength(2);
    const id = 'mock-conflict-entry';
    expect(mock.handle('sync_conflict_draft', { id, choices: ['both'] })).toContain(
      'felt great after the rain. legs heavy but good.',
    );
    mock.handle('sync_conflict_resolve', { id, choice: { kind: 'keep_other' } });
    expect((mock.handle('sync_conflicts', {}) as { needs_you: unknown[] }).needs_you).toHaveLength(
      1,
    );
    expect(() =>
      mock.handle('sync_conflict_resolve', { id, choice: { kind: 'keep_this' } }),
    ).toThrow();
  });

  it('resolving a Conflict logs "Conflict in … resolved" (M4)', () => {
    const log: Event[] = [];
    const mock = createConflictsMock(
      undefined,
      () => 1_000,
      undefined,
      (e) => log.push(e),
    );
    mock.handle('sync_conflict_resolve', {
      id: 'mock-conflict-notes',
      choice: { kind: 'keep_this' },
    });
    mock.handle('sync_conflict_resolve', {
      id: 'mock-conflict-entry',
      choice: { kind: 'merged', text: 'x' },
    });
    expect(
      log.map((e) =>
        lineOf(e, 'mock-me')
          .text.map((p) => p.text)
          .join(''),
      ),
    ).toEqual(['Conflict in a Task’s notes resolved', 'Conflict in an Entry’s text resolved']);
    // A Conflict that is already settled writes nothing more.
    expect(() =>
      mock.handle('sync_conflict_resolve', {
        id: 'mock-conflict-entry',
        choice: { kind: 'keep_this' },
      }),
    ).toThrow();
    expect(log).toHaveLength(2);
  });

  it('has no sample Conflicts or Resolved rows until a device is paired', () => {
    let paired = false;
    const mock = createConflictsMock(undefined, undefined, () => paired);
    const list = () =>
      mock.handle('sync_conflicts', {}) as { needs_you: unknown[]; resolved: unknown[] };
    expect(list()).toEqual({ needs_you: [], resolved: [] });
    expect(mock.openCount()).toBe(0);
    paired = true;
    expect(list().needs_you).toHaveLength(2);
    expect(mock.openCount()).toBe(2);
  });

  it('lists what was resolved automatically, new ones until visited, and Use other once', () => {
    const mock = createConflictsMock();
    type Resolved = { id: string; fresh: boolean; action: string | null; used: string | null };
    const resolved = () => (mock.handle('sync_conflicts', {}) as { resolved: Resolved[] }).resolved;
    const priority = () => resolved().find((r) => r.id === 'mock-resolved-priority');
    expect(priority()).toMatchObject({ fresh: true, action: 'Use P2 instead' });
    mock.handle('sync_resolutions_visited', {});
    expect(resolved().every((r) => !r.fresh)).toBe(true);
    mock.handle('sync_resolution_use_other', { id: 'mock-resolved-priority' });
    expect(priority()).toMatchObject({ action: null, used: 'Changed to P2' });
    expect(() =>
      mock.handle('sync_resolution_use_other', { id: 'mock-resolved-priority' }),
    ).toThrow();
  });

  it('builds Edit merged from the live text, so a later edit is kept', () => {
    const mock = createConflictsMock();
    let live = 'Run, felt great.\nTea.\n';
    mock.raise({
      row: { id: 'live' } as ConflictRow,
      base: 'Run.\nTea.\n',
      showing: live,
      other: 'Run, legs heavy.\nTea.\n',
      live: () => live,
      open: true,
      resolution: null,
    });
    live += 'Walk home.\n';
    expect(mock.handle('sync_conflict_draft', { id: 'live', choices: ['showing'] })).toBe(live);
    expect(mock.handle('sync_conflict_draft', { id: 'live', choices: ['other'] })).toBe(
      'Run, legs heavy.\nTea.\nWalk home.\n',
    );
    live = 'Run, felt really great.\nTea.\n';
    const view = mock.handle('sync_conflict', { id: 'live' }) as {
      pieces: { kind: string; showing?: string }[];
    };
    expect(view.pieces.find((p) => p.kind === 'clash')?.showing).toBe('felt really great.');
  });

  it('adds a Needs-you row each time a new Conflict arrives', () => {
    const emit = vi.fn();
    const mock = createConflictsMock(emit);
    const before = mock.openCount();
    const a = mock.raiseNew();
    const b = mock.raiseNew();
    expect(a.row.id).not.toBe(b.row.id);
    expect(mock.openCount()).toBe(before + 2);
    expect(emit).toHaveBeenCalledTimes(2);
    const list = mock.handle('sync_conflicts', {}) as { needs_you: { id: string }[] };
    expect(list.needs_you.map((r) => r.id)).toContain(a.row.id);
  });
});
