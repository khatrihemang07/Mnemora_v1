import { describe, expect, it } from 'vitest';
import type { ConflictRow } from '../../lib/ipc/bindings';
import { conflictIdOf, groupByDay, pageLabel, rowTitle, sidesText } from './conflicts';

const row = (id: string, raised_at: number): ConflictRow => ({
  id,
  subject: { kind: 'entry', page_date: '2026-10-21', written_time: '09:12' },
  module: 'journal',
  raised_at,
  showing: { device: 'Mac', at: raised_at },
  other: { device: 'Pixel 8', at: raised_at - 60_000 },
});

describe('conflict rows', () => {
  it('names what clashed and where', () => {
    expect(rowTitle(row('a', 0).subject)).toBe('Wed 21 Oct · 09:12 Entry');
    expect(rowTitle({ kind: 'task', title: 'Plan weekend hike', field: 'notes' })).toBe(
      '‘Plan weekend hike’ · notes',
    );
    expect(rowTitle({ kind: 'comment', task_title: 'Trip' })).toBe('‘Trip’ · comment');
    expect(pageLabel('nonsense')).toBe('nonsense');
  });

  it('lists the showing device first', () => {
    expect(sidesText(row('a', new Date(2026, 9, 21, 9, 40).getTime()))).toBe(
      'Mac 09:40 · Pixel 8 09:39',
    );
  });

  it('groups by day, newest first', () => {
    const now = new Date(2026, 9, 21, 12);
    const groups = groupByDay(
      [
        row('old', new Date(2026, 9, 19, 8).getTime()),
        row('a', new Date(2026, 9, 21, 9).getTime()),
        row('b', new Date(2026, 9, 21, 10).getTime()),
        row('y', new Date(2026, 9, 20, 10).getTime()),
      ],
      now,
    );
    expect(groups.map((g) => g.title)).toEqual(['Today', 'Yesterday', 'Mon']);
    expect(groups[0]?.rows.map((r) => r.id)).toEqual(['b', 'a']);
    expect(groups[0]?.subtitle).toBe('Wednesday 21 Oct');
  });

  it('reads the id from the address', () => {
    expect(conflictIdOf('/sync/conflicts/abc-1')).toBe('abc-1');
    expect(conflictIdOf('/sync/conflicts')).toBeNull();
  });
});
