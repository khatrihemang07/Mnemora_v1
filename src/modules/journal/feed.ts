import type { Page } from '../../lib/ipc/bindings';
import { monthLabel } from './dates';

/** One row of the virtual feed: a Page, or the faint divider before the first Page of a month. */
export type FeedItem =
  | { kind: 'page'; key: string; page: Page }
  | { kind: 'month'; key: string; label: string };

/**
 * Turns the loaded Pages (newest first) into feed rows. Today's Page is always there, empty if
 * nothing is written on it; a Journal with no Pages at all stays empty (the first-run state).
 * A month divider sits where the month changes, never above the first row.
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
