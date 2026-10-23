import { describe, expect, it } from 'vitest';
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
});
