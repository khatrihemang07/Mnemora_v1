import type { Page, PageDirection } from '../../lib/ipc/bindings';

/** Pages fetched per request. */
export const PAGE_SIZE = 30;

/** What one request asks `journal_pages` for. */
export interface PagesParam {
  cursor: string | null;
  direction: PageDirection;
}

/**
 * The first request of a window: the newest Pages, or the Pages starting AT `anchor` (a jump to an
 * old or far date). Older and newer Pages are fetched on from there.
 */
export const firstParam = (anchor: string | null): PagesParam => ({
  cursor: anchor,
  direction: 'from',
});

/**
 * The request for Pages older than the window, from its last group; undefined at the end.
 *
 * A short group means the end, except a `newer` group (always the first): it is short because the
 * top was reached, and an empty one carries no date, so the chain (which a refetch re-walks from
 * the first group) continues from its own cursor instead of stopping there and dropping the rest.
 */
export function olderParam(last: Page[], lastParam: PagesParam): PagesParam | undefined {
  if (lastParam.direction === 'newer') {
    const oldest = last.at(-1)?.date;
    if (oldest) return { cursor: oldest, direction: 'older' };
    return lastParam.cursor ? { cursor: lastParam.cursor, direction: 'from' } : undefined;
  }
  if (last.length < PAGE_SIZE) return undefined;
  return { cursor: last[last.length - 1]?.date ?? null, direction: 'older' };
}

/**
 * The request for Pages newer than the window, from its first group; undefined once the top is
 * reached. The newest window (no anchor) starts at the top already. After a jump the first group
 * cannot tell, so one more request is made; a short or empty answer ends it.
 */
export function newerParam(first: Page[], firstParam: PagesParam): PagesParam | undefined {
  const newest = first[0]?.date;
  if (!newest || firstParam.cursor === null) return undefined;
  if (firstParam.direction === 'newer' && first.length < PAGE_SIZE) return undefined;
  return { cursor: newest, direction: 'newer' };
}
