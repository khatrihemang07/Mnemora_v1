import type { Page } from '../../lib/ipc/bindings';
import { monthLabel } from './dates';

/** One row of the virtual feed: a Page, or the faint divider before the first Page of a month. */
export type FeedItem =
  | { kind: 'page'; key: string; page: Page }
  | { kind: 'month'; key: string; label: string };

/** How many future Pages (after `today`) are among the loaded Pages. */
export function upcomingCount(pages: Page[], today: string): number {
  return pages.filter((p) => p.date > today).length;
}

/**
 * Turns the loaded Pages (newest first) into feed rows. Today's Page is always there, empty if
 * nothing is written on it; a Journal with no Pages at all stays empty (the first-run state).
 * Future Pages sit above Today. A month divider sits where the month changes, never above the
 * first row.
 */
export function buildFeed(pages: Page[], today: string): FeedItem[] {
  if (pages.length === 0) return [];
  const all = pages.some((p) => p.date === today)
    ? pages
    : [...pages, { date: today, entry_count: 0, word_count: 0, entries: [] }].sort((a, b) =>
        b.date.localeCompare(a.date),
      );

  const items: FeedItem[] = [];
  let month: string | null = null;
  for (const page of all) {
    const m = page.date.slice(0, 7);
    if (month !== null && m !== month) {
      items.push({ kind: 'month', key: `month-${m}`, label: monthLabel(page.date) });
    }
    month = m;
    items.push({ kind: 'page', key: `page-${page.date}`, page });
  }
  return items;
}

/** Where the Journal opens: Today's Page when future Pages sit above it, else the top. */
export function openIndex(feed: FeedItem[], today: string): number {
  const first = feed[0];
  if (first?.kind !== 'page' || first.page.date <= today) return 0;
  return Math.max(0, pageIndex(feed, today));
}

/** Index of the Page row for `date`, or -1 when it is not loaded (yet). */
export function pageIndex(feed: FeedItem[], date: string): number {
  return feed.findIndex((i) => i.kind === 'page' && i.page.date === date);
}

/** The date of the Page being read at row `index`: the nearest Page row at or above it. */
export function pageAt(feed: FeedItem[], index: number): string | null {
  for (let i = Math.min(index, feed.length - 1); i >= 0; i--) {
    const item = feed[i];
    if (item?.kind === 'page') return item.page.date;
  }
  return null;
}

/**
 * What the Calendar opens on: the month of the Page in view with that day highlighted; Today when
 * no Page is in view.
 */
export function calendarStart(
  inView: string | null,
  today: string,
): { month: string; selected: string } {
  const date = inView ?? today;
  return { month: date.slice(0, 7), selected: date };
}
