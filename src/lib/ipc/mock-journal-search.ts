/**
 * Browser-mock Journal search. Follows the rules in crates/journal and crates/core/src/search (case and accents ignored, word
 * prefixes, every word required, "quoted phrases", typos by word length as a separate group) with
 * plain scans instead of an index, so the search UI can be tried without Rust.
 */
import type { AppError, Entry, Hit, SearchResults, Span } from './bindings';
import { validDate } from './mock-journal-pages';

const MAX_EXACT = 500;
const MAX_SIMILAR = 100;
/** Most near words kept per query word (MAX_NEAR_PER_WORD in crates/core/src/search/fuzzy.rs). */
const MAX_NEAR = 20;
const SNIPPET_CHARS = 160;
const LEAD_CHARS = 40;
const WORD = /[\p{L}\p{N}\p{M}]+/gu;

export type Term = { word: string } | { phrase: string[] };

export const fold = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
export const words = (s: string) => (s.match(WORD) ?? []).map(fold).filter(Boolean);

export function parse(query: string): Term[] {
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

/** Per term, the words of `vocab` (word -> Entries using it) a bare word may also match whole. */
export type Near = Set<string>[];

/**
 * Like `near_terms` in crates/core/src/search/fuzzy.rs: per bare word, the vocabulary words within its
 * tolerance (never the word itself), nearest first, then most used, then alphabetical, cut at
 * MAX_NEAR. Phrases and short words get none.
 */
export function nearWords(terms: Term[], vocab: Map<string, number>): Near {
  return terms.map((t) => {
    if (!('word' in t) || tolerance(t.word.length) === 0) return new Set();
    const max = tolerance(t.word.length);
    const found: [number, number, string][] = [];
    for (const [tok, docs] of vocab) {
      if (tok === t.word) continue;
      const d = distance(t.word, tok, max);
      if (d <= max) found.push([d, docs, tok]);
    }
    found.sort((a, b) => a[0] - b[0] || b[1] - a[1] || (a[2] < b[2] ? -1 : a[2] > b[2] ? 1 : 0));
    return new Set(found.slice(0, MAX_NEAR).map((f) => f[2]));
  });
}

function hasPhrase(tokens: string[], phrase: string[]): boolean {
  return tokens.some((_, i) => phrase.every((p, k) => tokens[i + k] === p));
}

/** Whether `tokens` satisfy every term; a bare word may also match one of its `near` words whole. */
export function matches(tokens: string[], terms: Term[], near: Near): boolean {
  return terms.every((t, i) =>
    'phrase' in t
      ? hasPhrase(tokens, t.phrase)
      : tokens.some((tok) => tok.startsWith(t.word) || near[i]?.has(tok)),
  );
}

/** Mirrors `plain_text` in crates/core/src/search/plain.rs: Markdown removed, lines joined with " · ". */
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
    .replace(/^([-*+]|\d+[.)])\s+/, '')
    .replace(/^\[[ xX]\]\s+/, '');
}

function snippet(body: string, terms: Term[], near: Near): Span[] {
  return windowSpans(plainText(body), terms, near);
}

/** Which words of `text` match: char ranges plus the folded word. */
function markedWords(text: string, terms: Term[], near: Near) {
  const hit = (tok: string) =>
    terms.some((t, i) =>
      'phrase' in t ? t.phrase.includes(tok) : tok.startsWith(t.word) || near[i]?.has(tok),
    );
  return [...text.matchAll(WORD)]
    .map((m) => ({ start: m.index ?? 0, end: (m.index ?? 0) + m[0].length, token: fold(m[0]) }))
    .filter((f) => hit(f.token));
}

/** Whether any word of `text` matches (Matcher::matches in crates/core/src/search/mark.rs). */
export const hasMarks = (text: string, terms: Term[], near: Near) =>
  markedWords(text, terms, near).length > 0;

/** Every word of `text` that matches, as spans covering all of `text` (a title to highlight). */
export function highlightSpans(text: string, terms: Term[], near: Near): Span[] {
  const spans: Span[] = [];
  let cursor = 0;
  for (const m of markedWords(text, terms, near)) {
    if (m.start > cursor) spans.push({ text: text.slice(cursor, m.start), matched: false });
    spans.push({ text: text.slice(m.start, m.end), matched: true });
    cursor = m.end;
  }
  if (cursor < text.length) spans.push({ text: text.slice(cursor), matched: false });
  return spans;
}

/** A window of plain `text` around its first match (`window` in crates/core/src/search/mark.rs). */
export function windowSpans(text: string, terms: Term[], near: Near): Span[] {
  const marks = markedWords(text, terms, near);
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
    .sort(
      (a, b) =>
        b.page_date.localeCompare(a.page_date) ||
        a.written_at - b.written_at ||
        (a.id < b.id ? -1 : a.id > b.id ? 1 : 0),
    );
  if (terms.length === 0) {
    const all = from === null && to === null ? [] : inRange;
    return {
      exact: all.slice(0, MAX_EXACT).map((e) => toHit(e, snippet(e.body, terms, []))),
      similar: [],
      truncated: all.length > MAX_EXACT,
      similar_truncated: false,
    };
  }
  const tokens = new Map(live.map((e) => [e.id, words(e.body)]));
  const vocab = new Map<string, number>();
  for (const toks of tokens.values()) {
    for (const tok of new Set(toks)) vocab.set(tok, (vocab.get(tok) ?? 0) + 1);
  }
  const near = nearWords(terms, vocab);
  const exact = inRange.filter((e) => matches(tokens.get(e.id) ?? [], terms, []));
  const exactIds = new Set(exact.map((e) => e.id));
  const similar = near.some((n) => n.size > 0)
    ? inRange.filter((e) => !exactIds.has(e.id) && matches(tokens.get(e.id) ?? [], terms, near))
    : [];
  return {
    exact: exact.slice(0, MAX_EXACT).map((e) => toHit(e, snippet(e.body, terms, []))),
    similar: similar.slice(0, MAX_SIMILAR).map((e) => toHit(e, snippet(e.body, terms, near))),
    truncated: exact.length > MAX_EXACT,
    similar_truncated: similar.length > MAX_SIMILAR,
  };
}
