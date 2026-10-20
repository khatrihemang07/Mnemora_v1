/**
 * In-memory Journal for the browser-only IPC mock. Mirrors the rules in crates/journal
 * (blank text rejected, valid dates, word count, ordering, moves, month days, search). Every result is a JSON copy, like real
 * IPC: handing out live objects would let the UI mutate the "database".
 */

import { toggleTaskLine } from '../ui/markdown/tasks';
import type { AppError, Entry, NewEntry, Page, PageDirection } from './bindings';
import { pageDates, validDate } from './mock-journal-pages';
import { plainText, searchEntries } from './mock-journal-search';
import { seedJournalEntries } from './mock-journal-seed';

const MAX_OFFSET_MIN = 14 * 60;
const MAX_ZONE_CHARS = 64;
const MAX_PAGES = 100;
const EDIT_GRACE_MS = 10 * 60_000;
const STYLE_MARKS = /[*_~=`]/g;
const TODAY_SNIPPET_CHARS = 140;

const fail = (code: AppError['code'], message: string): AppError => ({
  code,
  message,
  details: null,
});
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v));

function stripLinePrefix(line: string): string {
  return line
    .trimStart()
    .replace(/^(>\s*)+/, '')
    .replace(/^#+\s*/, '')
    .replace(/^([-*+]|\d+[.)])\s+/, '')
    .replace(/^\[[ xX]\]\s+/, '');
}

function wordCount(text: string): number {
  return text
    .split('\n')
    .flatMap((line) =>
      stripLinePrefix(line)
        .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
        .replace(STYLE_MARKS, ' ')
        .split(/\s+/),
    )
    .filter((w) => /[\p{L}\p{N}]/u.test(w)).length;
}

/** Mirrors `shorten` in crates/journal/src/today.rs: cut on a word, with an ellipsis. */
function shorten(text: string): string {
  const chars = [...text];
  if (chars.length <= TODAY_SNIPPET_CHARS) return text;
  let end = TODAY_SNIPPET_CHARS;
  while (end > 0 && chars[end] !== ' ') end--;
  if (end === 0) end = TODAY_SNIPPET_CHARS;
  return `${chars.slice(0, end).join('').trimEnd()}…`;
}

function mockId(): string {
  return crypto.randomUUID();
}

/** Switches to simulate failures. `?fail=journal_update` turns `failUpdate` on from the start. */
export interface JournalMockFlags {
  failUpdate: boolean;
}

/**
 * `seed` fills a sample Journal (dev only: `?seed=journal`); otherwise it starts empty. `flags` is
 * live: flip `failUpdate` to make saves fail, and back to let a Retry succeed.
 */
export function createJournalMock(seed = false, flags: JournalMockFlags = { failUpdate: false }) {
  const all: Entry[] = seed ? seedJournalEntries(new Date(), mockId) : [];
  /** id -> when it was deleted. Deleted Entries stay in `all` so a restore keeps their place. */
  const deleted = new Map<string, number>();
  const live = () => all.filter((e) => !deleted.has(e.id));
  const find = (id: unknown) => live().find((e) => e.id === id);
  const missing = () => fail('not_found', 'That Entry no longer exists');

  return {
    handles: (cmd: string) => cmd.startsWith('journal_'),

    handle(cmd: string, a: Record<string, unknown>): unknown {
      switch (cmd) {
        case 'journal_entry_create': {
          const n = a.entry as NewEntry;
          if (!n.text.trim()) throw fail('validation', "An Entry can't be blank");
          if (!validDate(n.page_date)) throw fail('validation', 'Not a valid date');
          if (!n.written_tz.trim() || [...n.written_tz].length > MAX_ZONE_CHARS) {
            throw fail('validation', 'Not a valid time zone');
          }
          if (Math.abs(n.written_offset_min) > MAX_OFFSET_MIN) {
            throw fail('validation', 'Not a valid time zone offset');
          }
          const now = Date.now();
          const entry: Entry = {
            id: mockId(),
            page_date: n.page_date,
            body: n.text,
            written_at: now,
            written_offset_min: n.written_offset_min,
            written_tz: n.written_tz,
            edited_at: null,
            word_count: wordCount(n.text),
            created_at: now,
            updated_at: now,
          };
          all.push(entry);
          return copy(entry);
        }
        case 'journal_entry_update': {
          if (flags.failUpdate) {
            throw {
              ...fail('database', 'Could not save the entry'),
              details: 'mock: simulated save failure',
            };
          }
          const text = a.text as string;
          if (!text.trim()) throw fail('validation', "An Entry can't be blank");
          const entry = find(a.id);
          if (!entry) throw missing();
          const now = Date.now();
          entry.body = text;
          entry.word_count = wordCount(text);
          entry.updated_at = now;
          // The last edit: moves on every update past 10 minutes after it was written.
          if (now - entry.written_at > EDIT_GRACE_MS) entry.edited_at = now;
          return copy(entry);
        }
        case 'journal_entry_toggle_task': {
          const entry = find(a.id);
          if (!entry) throw missing();
          const body = toggleTaskLine(entry.body, a.line as number);
          if (body === null) throw fail('validation', 'That line is not a checklist item');
          entry.body = body;
          entry.updated_at = Date.now();
          return copy(entry);
        }
        case 'journal_entry_move': {
          const pageDate = a.pageDate as string;
          if (!validDate(pageDate)) throw fail('validation', 'Not a valid date');
          const entry = find(a.id);
          if (!entry) throw missing();
          if (entry.page_date !== pageDate) {
            entry.page_date = pageDate;
            entry.updated_at = Date.now();
          }
          return copy(entry);
        }
        case 'journal_month_days': {
          const year = a.year as number;
          const month = a.month as number;
          if (!(year >= 1 && year <= 9999) || !(month >= 1 && month <= 12)) {
            throw fail('validation', 'Not a valid month');
          }
          const prefix = `${String(year).padStart(4, '0')}-${String(month).padStart(2, '0')}-`;
          return [
            ...new Set(
              live()
                .filter((e) => e.page_date.startsWith(prefix))
                .map((e) => e.page_date),
            ),
          ].sort();
        }
        case 'journal_entry_delete': {
          const entry = find(a.id);
          if (!entry) throw missing();
          const now = Date.now();
          entry.updated_at = now;
          deleted.set(entry.id, now);
          return null;
        }
        case 'journal_entry_restore': {
          const entry = all.find((e) => e.id === a.id && deleted.has(e.id));
          if (!entry) throw missing();
          deleted.delete(entry.id);
          entry.updated_at = Date.now();
          return copy(entry);
        }
        case 'journal_today': {
          const date = a.today as string;
          if (!validDate(date)) throw fail('validation', 'Not a valid date');
          const day = live().filter((e) => e.page_date === date);
          const last = day.reduce<Entry | null>(
            (best, e) => (best === null || e.written_at >= best.written_at ? e : best),
            null,
          );
          return copy({
            count: day.length,
            latest: last && {
              id: last.id,
              written_at: last.written_at,
              written_offset_min: last.written_offset_min,
              written_tz: last.written_tz,
              snippet: shorten(plainText(last.body)),
            },
          });
        }
        case 'journal_search':
          return copy(
            searchEntries(
              live(),
              a.query as string,
              a.from as string | null,
              a.to as string | null,
            ),
          );
        case 'journal_pages': {
          const cursor = a.cursor as string | null;
          if (cursor !== null && !validDate(cursor)) throw fail('validation', 'Not a valid date');
          const limit = Math.min(MAX_PAGES, Math.max(1, a.limit as number));
          const entries = live();
          const dates = pageDates(
            entries.map((e) => e.page_date),
            cursor,
            a.direction as PageDirection,
            limit,
          );
          if (dates === null) throw fail('validation', 'Newer Pages need a date to start from');
          const pages: Page[] = dates.map((date) => {
            const list = entries
              .filter((e) => e.page_date === date)
              .sort((x, y) => x.written_at - y.written_at);
            return {
              date,
              entry_count: list.length,
              word_count: list.reduce((n, e) => n + e.word_count, 0),
              entries: list,
            };
          });
          return copy(pages);
        }
        default:
          return undefined;
      }
    },
  };
}
