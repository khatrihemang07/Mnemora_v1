import { describe, expect, it } from 'vitest';
import type { ActivityRow } from '../../lib/ipc/bindings';
import {
  chipStillThere,
  chipsOf,
  estimate,
  expandable,
  filterOf,
  itemsOf,
  lineText,
} from './activity';

const row = (id: string, at: number, extra: Partial<ActivityRow> = {}): ActivityRow => ({
  id,
  kind: 'session',
  text: [
    { text: 'Synced with ', strong: false },
    { text: 'Pixel 8', strong: true },
    { text: ' · 12 in, 3 out', strong: false },
  ],
  on: null,
  at,
  breakdown: null,
  link: null,
  ...extra,
});

describe('Activity filters', () => {
  it('asks Rust for what the chip says', () => {
    expect(filterOf('all')).toEqual({ kind: 'all' });
    expect(filterOf('this')).toEqual({ kind: 'this_device' });
    expect(filterOf('dev-1')).toEqual({ kind: 'device', id: 'dev-1' });
  });

  it('offers All, This device, then each paired device', () => {
    expect(
      chipsOf([
        { id: 'a', name: 'Pixel 8' },
        { id: 'b', name: 'Air' },
      ]),
    ).toEqual([
      { value: 'all', label: 'All' },
      { value: 'this', label: 'This device' },
      { value: 'a', label: 'Pixel 8' },
      { value: 'b', label: 'Air' },
    ]);
    expect(chipsOf([])).toHaveLength(2);
  });

  it('falls back to All when the chosen device is gone', () => {
    expect(chipStillThere('a', [{ id: 'a' }])).toBe('a');
    expect(chipStillThere('a', [])).toBe('all');
    expect(chipStillThere('this', [])).toBe('this');
  });
});

describe('Activity grouping', () => {
  const now = new Date(2026, 9, 21, 12, 0);
  const at = (d: number, h: number, m = 0) => new Date(2026, 9, d, h, m).getTime();

  it('puts a serif heading before each day, newest first', () => {
    const items = itemsOf(
      [row('c', at(19, 8)), row('a', at(21, 9, 31)), row('b', at(21, 8, 2))],
      now,
    );
    expect(items.map((i) => (i.kind === 'day' ? `# ${i.title}` : i.row.id))).toEqual([
      '# Today',
      'a',
      'b',
      '# Monday',
      'c',
    ]);
    expect(items.find((i) => i.kind === 'day' && i.title === 'Monday')).toMatchObject({
      subtitle: '19 Oct',
    });
    const first = items[0];
    expect(first?.kind === 'day' && first.first).toBe(true);
    // Exactly Done's heading: the date beside it has no weekday.
    expect(first?.kind === 'day' && first.subtitle).toBe('21 Oct');
  });

  it('names yesterday and gives every heading and row its own key', () => {
    const items = itemsOf([row('a', at(21, 9)), row('b', at(20, 9))], now);
    expect(items.filter((i) => i.kind === 'day').map((i) => i.kind === 'day' && i.title)).toEqual([
      'Today',
      'Yesterday',
    ]);
    expect(new Set(items.map((i) => i.key)).size).toBe(items.length);
    expect(items.filter((i) => i.kind === 'day' && i.first)).toHaveLength(1);
  });

  it('has nothing for no rows', () => {
    expect(itemsOf([], now)).toEqual([]);
  });
});

describe('Activity rows', () => {
  it('reads as one sentence, with where it happened', () => {
    expect(lineText(row('a', 0))).toBe('Synced with Pixel 8 · 12 in, 3 out');
    expect(lineText(row('a', 0, { on: 'Air' }))).toBe(
      'Synced with Pixel 8 · 12 in, 3 out · on Air',
    );
  });

  it('opens only when there is a breakdown, and the guess grows with it', () => {
    const plain = row('a', 0);
    const session = row('b', 0, { breakdown: 'Todo: 9 Tasks, 1 List' });
    expect(expandable(plain)).toBe(false);
    expect(expandable(session)).toBe(true);
    expect(estimate({ kind: 'row', key: 'b', row: session }, true)).toBeGreaterThan(
      estimate({ kind: 'row', key: 'b', row: session }, false),
    );
    expect(estimate({ kind: 'row', key: 'a', row: plain }, true)).toBe(
      estimate({ kind: 'row', key: 'a', row: plain }, false),
    );
  });
});
