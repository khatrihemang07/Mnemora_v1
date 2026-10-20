import {
  type InfiniteData,
  infiniteQueryOptions,
  keepPreviousData,
  queryOptions,
} from '@tanstack/svelte-query';
import { commands, type Entry, type Page } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';
import { firstParam, newerParam, olderParam, PAGE_SIZE, type PagesParam } from './paging';

export { PAGE_SIZE };

export const journalKeys = {
  all: ['journal'] as const,
  /** Every feed window; one window per anchor date (null is the newest Pages). */
  pages: ['journal', 'pages'] as const,
  window: (anchor: string | null) => ['journal', 'pages', anchor] as const,
  search: (query: string, from: string | null, to: string | null) =>
    ['journal', 'search', query, from, to] as const,
  today: (date: string) => ['journal', 'today', date] as const,
  monthDays: (year: number, month: number) => ['journal', 'month-days', year, month] as const,
};

/** What the Home card shows about `date` (the user's local today): the count and the latest snippet. */
export const todayQuery = (date: string) =>
  queryOptions({
    queryKey: journalKeys.today(date),
    queryFn: () => unwrap(commands.journalToday(date)),
  });

/** Dates with Entries in a month (`month` is 1-12): the Calendar's dots. */
export const monthDaysQuery = (year: number, month: number) =>
  queryOptions({
    queryKey: journalKeys.monthDays(year, month),
    queryFn: () => unwrap(commands.journalMonthDays(year, month)),
    staleTime: 30_000,
  });

/**
 * Search results for `query` within the Page-date range. The previous results stay on screen while
 * the next ones load, so typing never flashes empty; `enabled` is the caller's to set.
 */
export const searchQuery = (query: string, range: { from: string; to: string } | null) =>
  queryOptions({
    queryKey: journalKeys.search(query, range?.from ?? null, range?.to ?? null),
    queryFn: () => unwrap(commands.journalSearch(query, range?.from ?? null, range?.to ?? null)),
    placeholderData: keepPreviousData,
    staleTime: 30_000,
  });

/**
 * The feed as a window of Pages, newest first. With no anchor it starts at the newest Pages and
 * pages older. With an anchor date (a jump to a far date) it starts AT that date, so only the Pages
 * around it load: older ones are fetched on scrolling down, newer ones (prepended) on scrolling up.
 * The previous window stays on screen while the next one loads.
 */
export const pagesQuery = (anchor: string | null) =>
  infiniteQueryOptions({
    queryKey: journalKeys.window(anchor),
    queryFn: ({ pageParam }) =>
      unwrap(commands.journalPages(pageParam.cursor, PAGE_SIZE, pageParam.direction)),
    initialPageParam: firstParam(anchor),
    getNextPageParam: (last, _all, lastParam: PagesParam) => olderParam(last, lastParam),
    getPreviousPageParam: (first, _all, firstParamValue: PagesParam) =>
      newerParam(first, firstParamValue),
    placeholderData: keepPreviousData,
  });

/** Where and when an Entry is written, taken from the device. */
export interface WrittenWhere {
  tz: string;
  offsetMin: number;
}

export function deviceWrittenWhere(now: Date): WrittenWhere {
  return {
    tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
    offsetMin: -now.getTimezoneOffset(),
  };
}

export function createEntry(
  pageDate: string,
  text: string,
  where: WrittenWhere,
  sessionId: string,
): Promise<Entry> {
  return unwrap(
    commands.journalEntryCreate({
      page_date: pageDate,
      text,
      written_tz: where.tz,
      written_offset_min: where.offsetMin,
      session_id: sessionId,
    }),
  );
}

export function updateEntry(id: string, text: string, sessionId: string): Promise<Entry> {
  return unwrap(commands.journalEntryUpdate(id, text, sessionId));
}

/**
 * Both writes leave the feed alone while the composer is open (it must not jump while you write);
 * the feed refreshes through this once the composer finishes.
 */
export function invalidateJournal(): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: journalKeys.all });
}

export function moveEntry(id: string, pageDate: string): Promise<Entry> {
  return unwrap(commands.journalEntryMove(id, pageDate));
}

export function toggleTask(id: string, line: number): Promise<Entry> {
  return unwrap(commands.journalEntryToggleTask(id, line));
}

export function deleteEntry(id: string): Promise<null> {
  return unwrap(commands.journalEntryDelete(id));
}

export function restoreEntry(id: string): Promise<Entry> {
  return unwrap(commands.journalEntryRestore(id));
}

/** Rewrites the cached Pages; `edit` returns the new Page, or null to drop it. */
function editCachedPages(edit: (page: Page) => Page | null): void {
  queryClient.setQueriesData<InfiniteData<Page[], PagesParam>>(
    { queryKey: journalKeys.pages },
    (data) => {
      if (!data) return data;
      return {
        ...data,
        pages: data.pages.map((group) => group.flatMap((page) => edit(page) ?? [])),
      };
    },
  );
}

const withEntries = (page: Page, entries: Entry[]): Page => ({
  ...page,
  entries,
  entry_count: entries.length,
  word_count: entries.reduce((n, e) => n + e.word_count, 0),
});

/**
 * Puts a saved Entry into the cached feed in place, without a refetch: inline editing saves while
 * the Entry is open, and the feed must neither jump nor flash old text when it closes.
 */
export function patchEntry(entry: Entry): void {
  editCachedPages((page) =>
    page.entries.some((e) => e.id === entry.id)
      ? withEntries(
          page,
          page.entries.map((e) => (e.id === entry.id ? entry : e)),
        )
      : page,
  );
}

/** Takes an Entry out of the cached feed at once (delete); a Page left empty goes with it. */
export function removeCachedEntry(id: string): void {
  editCachedPages((page) => {
    if (!page.entries.some((e) => e.id === id)) return page;
    const entries = page.entries.filter((e) => e.id !== id);
    return entries.length === 0 ? null : withEntries(page, entries);
  });
}
