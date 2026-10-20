import { type InfiniteData, infiniteQueryOptions } from '@tanstack/svelte-query';
import { commands, type Entry, type Page } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { unwrap } from '../../lib/ipc/unwrap';

/** Pages fetched per request. */
export const PAGE_SIZE = 30;

export const journalKeys = {
  all: ['journal'] as const,
  pages: ['journal', 'pages'] as const,
};

/** Pages newest first; the cursor is the previous request's last Page date. */
export const pagesQuery = infiniteQueryOptions({
  queryKey: journalKeys.pages,
  queryFn: ({ pageParam }) => unwrap(commands.journalPages(pageParam, PAGE_SIZE)),
  initialPageParam: null as string | null,
  getNextPageParam: (last) => (last.length < PAGE_SIZE ? undefined : last.at(-1)?.date),
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

/**
 * Puts a saved Entry into the cached feed in place, without a refetch: inline editing saves while
 * the Entry is open, and the feed must neither jump nor flash old text when it closes.
 */
export function patchEntry(entry: Entry): void {
  queryClient.setQueryData<InfiniteData<Page[]>>(journalKeys.pages, (data) => {
    if (!data) return data;
    return {
      ...data,
      pages: data.pages.map((group) =>
        group.map((page) => {
          if (!page.entries.some((e) => e.id === entry.id)) return page;
          const entries = page.entries.map((e) => (e.id === entry.id ? entry : e));
          return { ...page, entries, word_count: entries.reduce((n, e) => n + e.word_count, 0) };
        }),
      ),
    };
  });
}
