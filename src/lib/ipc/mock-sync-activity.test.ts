import { describe, expect, it } from 'vitest';
import type { ActivityFilter, ActivityPage } from './bindings';
import { createActivityMock } from './mock-sync-activity';

const NOW = new Date(2026, 9, 21, 12, 0).getTime();
const mock = () => createActivityMock(1200, NOW);
const page = (
  m: ReturnType<typeof mock>,
  filter: ActivityFilter,
  cursor: string | null,
  limit = 100,
) => m.handle('sync_activity', { filter, cursor, limit }) as ActivityPage;

function everything(m: ReturnType<typeof mock>, filter: ActivityFilter) {
  const rows = [];
  let cursor: string | null = null;
  do {
    const p: ActivityPage = page(m, filter, cursor);
    rows.push(...p.rows);
    cursor = p.next;
  } while (cursor);
  return rows;
}

describe('the Activity mock', () => {
  it('holds a long log, paged newest first without gaps or repeats', () => {
    const m = mock();
    const rows = everything(m, { kind: 'all' });
    expect(rows.length).toBeGreaterThan(1200);
    expect(new Set(rows.map((r) => r.id)).size).toBe(rows.length);
    expect(rows.every((r, i) => i === 0 || (rows[i - 1]?.at ?? 0) >= r.at)).toBe(true);
    expect(page(m, { kind: 'all' }, null).rows).toHaveLength(100);
  });

  it('offers Review only while the Conflict waits', () => {
    const open = new Set(['mock-conflict-notes', 'mock-conflict-entry']);
    const m = createActivityMock(1200, NOW, (id) => open.has(id));
    const links = () =>
      everything(m, { kind: 'all' })
        .filter((r) => r.link === 'conflicts')
        .map((r) => r.id);
    expect(links()).toHaveLength(2);
    open.delete('mock-conflict-notes');
    expect(links()).toHaveLength(1);
    open.clear();
    expect(links()).toHaveLength(0);
  });

  it('filters by device, and "this device" is the ones it took part in', () => {
    const m = mock();
    const all = everything(m, { kind: 'all' });
    const pixel = everything(m, { kind: 'device', id: 'mock-pixel' });
    const mine = everything(m, { kind: 'this_device' });
    expect(pixel.length).toBeGreaterThan(0);
    expect(pixel.length).toBeLessThan(all.length);
    expect(mine.length).toBeGreaterThan(pixel.length);
    expect(everything(m, { kind: 'device', id: 'nobody' })).toEqual([]);
  });

  it('breaks a session down into exactly as many records as it moved', () => {
    const rows = everything(mock(), { kind: 'all' }).filter((r) => r.kind === 'session');
    expect(rows.length).toBeGreaterThan(100);
    for (const r of rows) {
      const line = r.text.map((p) => p.text).join('');
      const m = /(\d+) in, (\d+) out/.exec(line);
      const moved = Number(m?.[1]) + Number(m?.[2]);
      // "Todo: 9 Tasks, 1 List · Journal: 2 Entries · Settings: 1": every number after a colon or comma.
      const counted = [...(r.breakdown ?? '').matchAll(/(?:: |, )(\d+)/g)].reduce(
        (sum, x) => sum + Number(x[1]),
        0,
      );
      expect(counted, `${line} / ${r.breakdown}`).toBe(moved);
    }
  });

  it('only offers Review on a Conflict that still waits', () => {
    const rows = everything(mock(), { kind: 'all' }).filter((r) => r.kind === 'conflict_raised');
    expect(rows.some((r) => r.link === 'conflicts')).toBe(true);
    expect(rows.some((r) => r.link === null)).toBe(true);
  });

  it('words an open Conflict line for what it is about', () => {
    const rows = everything(mock(), { kind: 'all' }).filter((r) => r.kind === 'conflict_raised');
    const open = rows
      .filter((r) => r.link === 'conflicts')
      .map((r) => r.text.map((p) => p.text).join(''));
    expect(open.some((t) => t.includes('Entry’s text'))).toBe(true);
    expect(open.some((t) => t.includes('Task’s notes'))).toBe(true);
  });

  it('words a session as Rust does, and sessions of others carry where they happened', () => {
    const m = mock();
    const rows = everything(m, { kind: 'all' });
    const own = rows.find((r) => r.kind === 'session' && r.on === null);
    const theirs = rows.find((r) => r.kind === 'session' && r.on !== null);
    expect(own?.text.map((p) => p.text).join('')).toMatch(/^Synced with .+ · \d+ in, \d+ out$/);
    expect(own?.breakdown).toMatch(/:/);
    expect(theirs?.on).toMatch(/simulated/);
  });

  it('refuses a cursor it cannot read', () => {
    expect(() => page(mock(), { kind: 'all' }, 'junk')).toThrow();
  });
});
