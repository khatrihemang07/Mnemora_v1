/**
 * In-memory Journal for the browser-only IPC mock. Mirrors the rules in crates/journal
 * (blank text rejected, valid dates, word count, ordering). Every result is a JSON copy, like real
 * IPC: handing out live objects would let the UI mutate the "database".
 */
import type { AppError, Entry, NewEntry, Page } from './bindings';
import { seedJournalEntries } from './mock-journal-seed';

const MAX_OFFSET_MIN = 14 * 60;
const MAX_ZONE_CHARS = 64;
const MAX_PAGES = 100;
const STYLE_MARKS = /[*_~=`]/g;

const fail = (code: AppError['code'], message: string): AppError => ({
  code,
  message,
  details: null,
});
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v));

function validDate(date: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return false;
  const [y = 0, m = 1, d = 1] = date.split('-').map(Number);
  const probe = new Date(Date.UTC(y, m - 1, d));
  return (
    y >= 1 &&
    probe.getUTCFullYear() === y &&
    probe.getUTCMonth() === m - 1 &&
    probe.getUTCDate() === d
  );
}

function stripLinePrefix(line: string): string {
  return line
    .trimStart()
    .replace(/^(>\s*)+/, '')
    .replace(/^#+\s*/, '')
    .replace(/^([-*+]|\d+\.)\s+/, '')
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

function mockId(): string {
  return crypto.randomUUID();
}

/** `seed` fills a sample Journal (dev only: `?seed=journal`); otherwise it starts empty. */
export function createJournalMock(seed = false) {
  const entries: Entry[] = seed ? seedJournalEntries(new Date(), mockId) : [];

  return {
    handles: (cmd: string) => cmd.startsWith('journal_'),

    handle(cmd: string, a: Record<string, unknown>): unknown {
      switch (cmd) {
        case 'journal_entry_create': {
          const n = a.entry as NewEntry;
          if (!n.text.trim()) throw fail('validation', "An Entry can't be blank");
          if (!validDate(n.page_date)) throw fail('validation', 'Not a valid date');
          if (
            !n.written_tz.trim() ||
            [...n.written_tz].length > MAX_ZONE_CHARS ||
            Math.abs(n.written_offset_min) > MAX_OFFSET_MIN
          ) {
            throw fail('validation', 'Not a valid time zone');
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
          entries.push(entry);
          return copy(entry);
        }
        case 'journal_entry_update': {
          const text = a.text as string;
          if (!text.trim()) throw fail('validation', "An Entry can't be blank");
          const entry = entries.find((e) => e.id === a.id);
          if (!entry) throw fail('unknown_key', 'That Entry no longer exists');
          entry.body = text;
          entry.word_count = wordCount(text);
          entry.updated_at = Date.now();
          return copy(entry);
        }
        case 'journal_pages': {
          const cursor = a.cursor as string | null;
          if (cursor !== null && !validDate(cursor)) throw fail('validation', 'Not a valid date');
          const limit = Math.min(MAX_PAGES, Math.max(1, a.limit as number));
          const dates = [...new Set(entries.map((e) => e.page_date))]
            .filter((d) => cursor === null || d < cursor)
            .sort()
            .reverse()
            .slice(0, limit);
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
