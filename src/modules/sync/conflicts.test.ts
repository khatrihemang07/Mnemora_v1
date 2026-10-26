import { describe, expect, it } from 'vitest';
import type { ConflictRow } from '../../lib/ipc/bindings';
import { conflictIdOf, groupByDay, pageLabel, rowTitle, sidesText, timeText } from './conflicts';

const row = (id: string, raised_at: number): ConflictRow => ({
  id,
  subject: {
    kind: 'entry',
    page_date: '2026-10-21',
    written_at: Date.UTC(2026, 9, 21, 9, 12),
    written_offset_min: 0,
  },
  module: 'journal',
  raised_at,
  showing: { device: 'Mac', at: raised_at },
  other: { device: 'Pixel 8', at: raised_at - 60_000 },
});

describe('conflict rows', () => {
  it('names an AI title or Saved question that clashed', () => {
    expect(rowTitle({ kind: 'conversation', title: 'Plan the hike' }, '24h')).toBe(
      '‘Plan the hike’ · conversation title',
    );
    expect(rowTitle({ kind: 'saved_question', text: 'Coffee shops' }, '24h')).toBe(
      '‘Coffee shops’ · saved question',
    );
  });

  it('names what clashed and where', () => {
    expect(rowTitle(row('a', 0).subject, '24h')).toBe('Wed 21 Oct · 09:12 Entry');
    expect(rowTitle(row('a', 0).subject, '12h')).toBe('Wed 21 Oct · 9:12 AM Entry');
    // Written at 23:30 where it is UTC-5 (04:30 UTC the next day): the time there, not here.
    const late = {
      kind: 'entry' as const,
      page_date: '2026-10-21',
      written_at: Date.UTC(2026, 9, 22, 4, 30),
      written_offset_min: -300,
    };
    expect(rowTitle(late, '24h')).toBe('Wed 21 Oct · 23:30 Entry');
    expect(rowTitle(late, '12h')).toBe('Wed 21 Oct · 11:30 PM Entry');
    expect(rowTitle({ kind: 'task', title: 'Plan weekend hike', field: 'notes' }, '24h')).toBe(
      '‘Plan weekend hike’ · notes',
    );
    expect(rowTitle({ kind: 'comment', task_title: 'Trip' }, '24h')).toBe('‘Trip’ · comment');
    expect(pageLabel('nonsense')).toBe('nonsense');
  });

  it('lists the showing device first', () => {
    const r = row('a', new Date(2026, 9, 21, 21, 40).getTime());
    expect(sidesText(r, '24h')).toBe('Mac 21:40 · Pixel 8 21:39');
    expect(sidesText(r, '12h')).toBe('Mac 9:40 PM · Pixel 8 9:39 PM');
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
    // The same heading Done and Activity have: the day's name, the small date beside it.
    expect(groups.map((g) => [g.title, g.subtitle])).toEqual([
      ['Today', '21 Oct'],
      ['Yesterday', '20 Oct'],
      ['Monday', '19 Oct'],
    ]);
    expect(groups[0]?.rows.map((r) => r.id)).toEqual(['b', 'a']);
  });

  it('groups any rows by the time it is given, Resolved automatically by when it was decided', () => {
    const now = new Date(2026, 9, 21, 12);
    const rows = [
      { id: 'a', at: new Date(2026, 9, 21, 9).getTime() },
      { id: 'b', at: new Date(2026, 9, 20, 9).getTime() },
    ];
    const groups = groupByDay(rows, now, (r) => r.at);
    expect(groups.map((g) => [g.title, g.rows.map((r) => r.id)])).toEqual([
      ['Today', ['a']],
      ['Yesterday', ['b']],
    ]);
  });

  it('writes a session time the way the Time format says', () => {
    const t = new Date(2026, 9, 21, 15, 7).getTime();
    expect(timeText(t, '24h')).toBe('15:07');
    expect(timeText(t, '12h')).toBe('3:07 PM');
  });

  it('reads the id from the address', () => {
    expect(conflictIdOf('/sync/conflicts/abc-1')).toBe('abc-1');
    expect(conflictIdOf('/sync/conflicts')).toBeNull();
  });
});
