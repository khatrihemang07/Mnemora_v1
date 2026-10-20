import { describe, expect, it } from 'vitest';
import type { Page } from '../../lib/ipc/bindings';
import {
  buildFeed,
  calendarStart,
  type FeedItem,
  openIndex,
  pageAt,
  pageIndex,
  upcomingCount,
} from './feed';

const page = (date: string, entry_count = 1): Page => ({
  date,
  entry_count,
  word_count: entry_count * 3,
  entries: [],
});
const kinds = (items: FeedItem[]) =>
  items.map((i) => (i.kind === 'page' ? i.page.date : `# ${i.label}`));

describe('buildFeed', () => {
  it('is empty on a first run, so the empty state can show', () => {
    expect(buildFeed([], '2026-10-20')).toEqual([]);
  });
  it('always has Today, empty when nothing is written on it', () => {
    const items = buildFeed([page('2026-10-19')], '2026-10-20');
    expect(kinds(items)).toEqual(['2026-10-20', '2026-10-19']);
    const first = items[0];
    expect(first?.kind === 'page' && first.page.entry_count).toBe(0);
  });
  it('leaves Today out while the window has not reached it', () => {
    expect(kinds(buildFeed([page('2025-03-02'), page('2025-03-01')], '2026-10-20', false))).toEqual(
      ['2025-03-02', '2025-03-01'],
    );
  });
  it('does not duplicate a Today that has Entries', () => {
    expect(kinds(buildFeed([page('2026-10-20'), page('2026-10-19')], '2026-10-20'))).toEqual([
      '2026-10-20',
      '2026-10-19',
    ]);
  });
  it('slots Today below future Pages, with a month divider where the month changes', () => {
    expect(kinds(buildFeed([page('2026-10-25'), page('2026-10-19')], '2026-10-20'))).toEqual([
      '2026-10-25',
      '2026-10-20',
      '2026-10-19',
    ]);
    expect(
      kinds(buildFeed([page('2026-11-02'), page('2026-10-20'), page('2026-10-19')], '2026-10-20')),
    ).toEqual(['2026-11-02', '# October 2026', '2026-10-20', '2026-10-19']);
  });
  it('puts a month divider before the first Page of an older month, never at the top', () => {
    const items = buildFeed(
      [
        page('2026-10-20'),
        page('2026-10-02'),
        page('2026-09-30'),
        page('2026-09-01'),
        page('2026-08-15'),
      ],
      '2026-10-20',
    );
    expect(kinds(items)).toEqual([
      '2026-10-20',
      '2026-10-02',
      '# September 2026',
      '2026-09-30',
      '2026-09-01',
      '# August 2026',
      '2026-08-15',
    ]);
  });
  it('keys items uniquely', () => {
    const items = buildFeed([page('2026-10-20'), page('2025-01-01')], '2026-10-20');
    expect(new Set(items.map((i) => i.key)).size).toBe(items.length);
  });
});

describe('upcomingCount', () => {
  it('counts only Pages after today', () => {
    const pages = [page('2027-01-01'), page('2026-10-21'), page('2026-10-20'), page('2026-10-19')];
    expect(upcomingCount(pages, '2026-10-20')).toBe(2);
    expect(upcomingCount([], '2026-10-20')).toBe(0);
    expect(upcomingCount([page('2026-10-20')], '2026-10-20')).toBe(0);
  });
  it('never counts the empty Today the feed adds', () => {
    const items = buildFeed([page('2026-10-22'), page('2026-10-21')], '2026-10-20');
    const pages = items.flatMap((i) => (i.kind === 'page' ? [i.page] : []));
    expect(upcomingCount(pages, '2026-10-20')).toBe(2);
  });
});

describe('opening, finding and reading Pages', () => {
  const feed = buildFeed(
    [page('2026-10-22'), page('2026-10-21'), page('2026-10-20'), page('2026-09-30')],
    '2026-10-20',
  );
  it('opens on Today when future Pages exist, else at the top', () => {
    expect(openIndex(feed, '2026-10-20')).toBe(2);
    expect(openIndex(buildFeed([page('2026-10-20')], '2026-10-20'), '2026-10-20')).toBe(0);
    expect(openIndex([], '2026-10-20')).toBe(0);
  });
  it('finds a Page row by date', () => {
    expect(pageIndex(feed, '2026-10-20')).toBe(2);
    expect(pageIndex(feed, '2020-01-01')).toBe(-1);
  });
  it('reads the nearest Page at or above a row, skipping dividers', () => {
    expect(pageAt(feed, 2)).toBe('2026-10-20');
    expect(pageAt(feed, 3)).toBe('2026-10-20'); // the September divider
    expect(pageAt(feed, 4)).toBe('2026-09-30');
    expect(pageAt(feed, 99)).toBe('2026-09-30');
    expect(pageAt([], 0)).toBeNull();
  });
});

describe('calendarStart', () => {
  it('opens on the month of the Page in view with that day selected', () => {
    expect(calendarStart('2026-09-30', '2026-10-20')).toEqual({
      month: '2026-09',
      selected: '2026-09-30',
    });
  });
  it('falls back to Today', () => {
    expect(calendarStart(null, '2026-10-20')).toEqual({ month: '2026-10', selected: '2026-10-20' });
  });
});
