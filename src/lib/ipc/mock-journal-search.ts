/**
 * Browser-mock Journal search. Follows the rules in crates/journal (case and accents ignored, word
 * prefixes, every word required, "quoted phrases", typos by word length as a separate group) with
 * plain scans instead of an index, so the search UI can be tried without Rust.
 */
import type { AppError, Entry, Hit, SearchResults, Span } from './bindings';

const MAX_EXACT = 500;
const MAX_SIMILAR = 100;
const SNIPPET_CHARS = 160;
const LEAD_CHARS = 40;
const WORD = /[\p{L}\p{N}\p{M}]+/gu;

type Term = { word: string } | { phrase: string[] };

const fold = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
const words = (s: string) => (s.match(WORD) ?? []).map(fold).filter(Boolean);

function parse(query: string): Term[] {
  return query.split('"').flatMap((part, i): Term[] => {
    const ws = words(part);
    if (i % 2 === 1) return ws.length ? [{ phrase: ws }] : [];
    return ws.map((word) => ({ word }));
  });
}

const tolerance = (len: number) => (len < 4 ? 0 : len < 8 ? 1 : 2);

/** Levenshtein distance, or max + 1 once it is certain to exceed `max`. */
function distance(a: string, b: string, max: number): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const row = [i];
    for (let j = 1; j <= b.length; j++) {
      row[j] = Math.min(
        (prev[j - 1] ?? 0) + (a[i - 1] === b[j - 1] ? 0 : 1),
        (prev[j] ?? 0) + 1,
        (row[j - 1] ?? 0) + 1,
      );
    }
    if (Math.min(...row) > max) return max + 1;
    prev = row;
  }
  return prev[b.length] ?? max + 1;
}

const near = (token: string, word: string) =>
  token !== word && distance(token, word, tolerance(word.length)) <= tolerance(word.length);

function hasPhrase(tokens: string[], phrase: string[]): boolean {
  return tokens.some((_, i) => phrase.every((p, k) => tokens[i + k] === p));
}

/** Whether `tokens` satisfy every term; `fuzzy` lets a bare word match a near token too. */
function matches(tokens: string[], terms: Term[], fuzzy: boolean): boolean {
  return terms.every((t) =>
    'phrase' in t
      ? hasPhrase(tokens, t.phrase)
      : tokens.some(
          (tok) =>
            tok.startsWith(t.word) || (fuzzy && tolerance(t.word.length) > 0 && near(tok, t.word)),
        ),
  );
}

/** Mirrors `plain_text` in crates/journal/src/words.rs: Markdown removed, lines joined with " · ". */
export function plainText(body: string): string {
  let inFence = false;
  const lines: string[] = [];
  for (const line of body.split('\n')) {
    if (line.trimStart().startsWith('```')) {
      inFence = !inFence;
      continue;
    }
    const plain = inFence
      ? line.trim()
      : stripLinePrefix(line)
          .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
          .replace(/~~|==|[*`]/g, '')
          .replace(/(?<![\p{L}\p{N}])_|_(?![\p{L}\p{N}])/gu, '');
    if (/[\p{L}\p{N}]/u.test(plain)) lines.push(plain.split(/\s+/).filter(Boolean).join(' '));
  }
  return lines.join(' · ');
}

function stripLinePrefix(line: string): string {
  return line
    .trimStart()
    .replace(/^(>\s*)+/, '')
    .replace(/^#+\s*/, '')
    .replace(/^([-*+]|\d+\.)\s+/, '')
    .replace(/^\[[ xX]\]\s+/, '');
}

function snippet(body: string, terms: Term[], fuzzy: boolean): Span[] {
  const text = plainText(body);
  const found = [...text.matchAll(WORD)].map((m) => ({
    start: m.index ?? 0,
    end: (m.index ?? 0) + m[0].length,
    token: fold(m[0]),
  }));
  const hit = (tok: string) =>
    terms.some((t) =>
      'phrase' in t
        ? t.phrase.includes(tok)
        : tok.startsWith(t.word) || (fuzzy && tolerance(t.word.length) > 0 && near(tok, t.word)),
    );
  const marks = found.filter((f) => hit(f.token));
  const first = marks[0]?.start ?? 0;
  let start = Math.max(0, first - LEAD_CHARS);
  while (start > 0 && start < first && text[start - 1] !== ' ') start++;
  let end = Math.min(text.length, start + SNIPPET_CHARS);
  const straddle = marks.find((m) => m.start < end && m.end > end);
  if (straddle) end = straddle.end;
  else {
    const limit = end;
    while (end > start && end < text.length && text[end] !== ' ') end--;
    if (end === start) end = limit;
  }
  const spans: Span[] = [];
  if (start > 0) spans.push({ text: '…', matched: false });
  let cursor = start;
  for (const m of marks.filter((x) => x.start >= start && x.end <= end)) {
    if (m.start > cursor) spans.push({ text: text.slice(cursor, m.start), matched: false });
    spans.push({ text: text.slice(m.start, m.end), matched: true });
    cursor = m.end;
  }
  const tail = text.slice(cursor, end);
  const tailText = end < text.length ? `${tail.trimEnd()}…` : tail;
  if (tailText) spans.push({ text: tailText, matched: false });
  return spans;
}

const validDate = (d: string) => /^\d{4}-\d{2}-\d{2}$/.test(d) && !Number.isNaN(Date.parse(d));

function toHit(e: Entry, spans: Span[]): Hit {
  return {
    id: e.id,
    page_date: e.page_date,
    written_at: e.written_at,
    written_offset_min: e.written_offset_min,
    written_tz: e.written_tz,
    snippet: spans,
  };
}

/** Searches the live Entries; the result is already a fresh object (callers copy it anyway). */
export function searchEntries(
  live: Entry[],
  query: string,
  from: string | null,
  to: string | null,
): SearchResults {
  for (const d of [from, to]) {
    if (d !== null && !validDate(d)) {
      throw { code: 'validation', message: 'Not a valid date', details: null } satisfies AppError;
    }
  }
  if (from !== null && to !== null && from > to) {
    throw {
      code: 'validation',
      message: 'The start date is after the end date',
      details: null,
    } satisfies AppError;
  }
  const terms = parse(query);
  const inRange = live
    .filter((e) => (from === null || e.page_date >= from) && (to === null || e.page_date <= to))
    .sort((a, b) => b.page_date.localeCompare(a.page_date) || a.written_at - b.written_at);
  if (terms.length === 0) {
    const all = from === null && to === null ? [] : inRange;
    return {
      exact: all.slice(0, MAX_EXACT).map((e) => toHit(e, snippet(e.body, terms, false))),
      similar: [],
      truncated: all.length > MAX_EXACT,
    };
  }
  const tokens = new Map(inRange.map((e) => [e.id, words(e.body)]));
  const exact = inRange.filter((e) => matches(tokens.get(e.id) ?? [], terms, false));
  const exactIds = new Set(exact.map((e) => e.id));
  const similar = inRange.filter(
    (e) => !exactIds.has(e.id) && matches(tokens.get(e.id) ?? [], terms, true),
  );
  return {
    exact: exact.slice(0, MAX_EXACT).map((e) => toHit(e, snippet(e.body, terms, false))),
    similar: similar.slice(0, MAX_SIMILAR).map((e) => toHit(e, snippet(e.body, terms, true))),
    truncated: exact.length > MAX_EXACT,
  };
}
