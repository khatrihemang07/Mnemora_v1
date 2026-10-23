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
