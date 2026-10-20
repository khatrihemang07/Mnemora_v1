import { describe, expect, it } from 'vitest';
import type { Page } from '../../lib/ipc/bindings';
import { firstParam, newerParam, olderParam, PAGE_SIZE } from './paging';

const page = (date: string): Page => ({ date, entry_count: 1, word_count: 1, entries: [] });
const full = (): Page[] =>
  Array.from({ length: PAGE_SIZE }, (_, i) => page(`2026-01-${String(30 - i).padStart(2, '0')}`));

describe('firstParam', () => {
  it('starts at the newest Pages, or AT the anchor date', () => {
    expect(firstParam(null)).toEqual({ cursor: null, direction: 'from' });
    expect(firstParam('2025-03-02')).toEqual({ cursor: '2025-03-02', direction: 'from' });
  });
});

describe('olderParam', () => {
  it('continues before the last Page of a full group', () => {
    expect(olderParam(full(), firstParam(null))).toEqual({
      cursor: '2026-01-01',
      direction: 'older',
    });
  });

  it('ends on a short group', () => {
    expect(olderParam([page('2026-01-01')], { cursor: '2026-01-05', direction: 'older' })).toBe(
      undefined,
    );
  });

  it('keeps going after a short newer group (the top, not the end)', () => {
    expect(
      olderParam([page('2026-02-02'), page('2026-02-01')], {
        cursor: '2026-01-30',
        direction: 'newer',
      }),
    ).toEqual({ cursor: '2026-02-01', direction: 'older' });
  });

  it('keeps going after an empty newer group, from its own cursor', () => {
    expect(olderParam([], { cursor: '2026-01-30', direction: 'newer' })).toEqual({
      cursor: '2026-01-30',
      direction: 'from',
    });
  });
});

describe('newerParam', () => {
  it('has nothing above the newest window', () => {
    expect(newerParam(full(), firstParam(null))).toBe(undefined);
  });

  it('tries once above an anchored window, from its newest Page', () => {
    expect(newerParam([page('2025-03-02'), page('2025-03-01')], firstParam('2025-03-02'))).toEqual({
      cursor: '2025-03-02',
      direction: 'newer',
    });
  });

  it('continues above a full newer group and stops at a short or empty one', () => {
    const param = { cursor: '2025-03-02', direction: 'newer' as const };
    expect(newerParam(full(), param)).toEqual({ cursor: '2026-01-30', direction: 'newer' });
    expect(newerParam([page('2026-01-30')], param)).toBe(undefined);
    expect(newerParam([], param)).toBe(undefined);
  });
});
