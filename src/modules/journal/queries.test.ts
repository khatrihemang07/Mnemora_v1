import { InfiniteQueryObserver, QueryClient } from '@tanstack/svelte-query';
import { describe, expect, it, vi } from 'vitest';
import type { Page, PageDirection } from '../../lib/ipc/bindings';
import { pageDates } from '../../lib/ipc/mock-journal-pages';

// A backend of one Page per day, `DAYS` of them, behind the real paging rules.
const DAYS = 100;
const dateOf = (n: number) => new Date(Date.UTC(2026, 0, 1 + n)).toISOString().slice(0, 10);
const ALL = Array.from({ length: DAYS }, (_, n) => dateOf(n));
const requests: [string | null, PageDirection][] = [];

vi.mock('../../lib/ipc/bindings', () => ({
  commands: {
    journalPages: async (cursor: string | null, limit: number, direction: PageDirection) => {
      requests.push([cursor, direction]);
      const dates = pageDates(ALL, cursor, direction, limit) ?? [];
      const data: Page[] = dates.map((date) => ({
        date,
        entry_count: 1,
        word_count: 1,
        entries: [],
      }));
      return { status: 'ok', data };
    },
  },
}));

const { PAGE_SIZE, pagesQuery } = await import('./queries');

const observe = (anchor: string | null) =>
  new InfiniteQueryObserver(new QueryClient(), pagesQuery(anchor));
const dates = (r: { data?: { pages: Page[][] } }) =>
  (r.data?.pages.flat() ?? []).map((p) => p.date);
const strictlyNewestFirst = (list: string[]) =>
  list.every((d, i) => i === 0 || (list[i - 1] ?? '') > d);

describe('feed window paging', () => {
  it('opens AT the anchor: only the Pages from there down load, not everything above', async () => {
    const o = observe(dateOf(50));
    await o.refetch();
    const first = o.getCurrentResult();
    expect(dates(first)).toEqual(ALL.slice(0, 51).reverse().slice(0, PAGE_SIZE));
    expect(first.hasPreviousPage).toBe(true);
    expect(first.hasNextPage).toBe(true);
  });

  it('opens at the newest Pages with nothing above', async () => {
    const o = observe(null);
    await o.refetch();
    const first = o.getCurrentResult();
    expect(dates(first)[0]).toBe(dateOf(DAYS - 1));
    expect(first.hasPreviousPage).toBe(false);
  });

  it('prepends newer Pages in order until the top, then stops', async () => {
    const o = observe(dateOf(10));
    await o.refetch();
    let last = await o.fetchPreviousPage();
    expect(dates(last)).toHaveLength(11 + PAGE_SIZE); // the 11 from the anchor down, 30 above
    expect(strictlyNewestFirst(dates(last))).toBe(true);
    for (let i = 0; i < 5 && last.hasPreviousPage; i++) last = await o.fetchPreviousPage();
    expect(last.hasPreviousPage).toBe(false);
    expect(dates(last)).toEqual([...ALL].reverse());
  });

  it('keeps every group, in order and without repeats, when refetched after prepending', async () => {
    const o = observe(dateOf(40));
    await o.refetch();
    await o.fetchPreviousPage();
    await o.fetchNextPage();
    const before = dates(o.getCurrentResult());
    const after = dates(await o.refetch());
    expect(after).toEqual(before);
    expect(strictlyNewestFirst(after)).toBe(true);
  });

  it('survives an empty newer group at the exact top (refetch keeps the older groups)', async () => {
    // Anchor so exactly PAGE_SIZE Pages sit above it: the next newer request comes back empty.
    const o = observe(dateOf(DAYS - 1 - PAGE_SIZE));
    await o.refetch();
    await o.fetchPreviousPage(); // exactly the 30 above
    const empty = await o.fetchPreviousPage();
    expect(empty.hasPreviousPage).toBe(false);
    const before = dates(empty);
    expect(before[0]).toBe(dateOf(DAYS - 1));
    const after = dates(await o.refetch());
    expect(after).toEqual(before);
  });
});
