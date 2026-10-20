/**
 * Runs the shared rule fixtures (fixtures/journal-rules.json) against the browser mock. The Rust
 * commands run the same file in src-tauri/src/journal.rs, so the mock cannot drift from them
 * (ADR 0009). Keep `normalise` in step with `normalise` there.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Entry, SearchResults, Span } from './bindings';
import rules from './fixtures/journal-rules.json';
import { createJournalMock } from './mock-journal';

interface Seed {
  body: string;
  page_date?: string;
  at?: number;
  id_n?: number;
  repeat?: number;
}
interface Case {
  rule: string;
  name: string;
  seed: Seed[];
  age_minutes?: number;
  op: string;
  args: Record<string, unknown>;
  expect: Record<string, unknown>;
}

const START = Date.UTC(2026, 9, 20, 12, 0, 0);
const idOf = (n: number) => `id-${String(n).padStart(4, '0')}`;
const command = (op: string) =>
  ['create', 'update', 'toggle_task', 'move'].includes(op)
    ? `journal_entry_${op}`
    : `journal_${op}`;

const spans = (list: Span[]) => list.map((s) => [s.text, s.matched]);

/** The comparable shape of a result: Entries by seed index, no ids or timestamps. */
function normalise(op: string, result: unknown, index: Map<string, number>): unknown {
  const at = (id: string) => index.get(id) ?? id;
  const entry = (e: Entry) => ({
    body: e.body,
    page_date: e.page_date,
    word_count: e.word_count,
    edited: e.edited_at !== null,
  });
  switch (op) {
    case 'search': {
      const r = result as SearchResults;
      return {
        exact: r.exact.map((h) => at(h.id)),
        similar: r.similar.map((h) => at(h.id)),
        exact_count: r.exact.length,
        similar_count: r.similar.length,
        truncated: r.truncated,
        similar_truncated: r.similar_truncated,
        exact_spans: r.exact.map((h) => spans(h.snippet)),
        similar_spans: r.similar.map((h) => spans(h.snippet)),
      };
    }
    case 'create':
      return { created: entry(result as Entry) };
    case 'update':
    case 'toggle_task':
    case 'move':
      return { entry: entry(result as Entry) };
    case 'today': {
      const t = result as { count: number; latest: { id: string; snippet: string } | null };
      return {
        count: t.count,
        latest: t.latest ? at(t.latest.id) : null,
        snippet: t.latest ? t.latest.snippet : null,
      };
    }
    case 'pages': {
      const pages = result as {
        date: string;
        entry_count: number;
        word_count: number;
        entries: Entry[];
      }[];
      return {
        pages: pages.map((p) => ({
          date: p.date,
          entries: p.entries.map((e) => at(e.id)),
          entry_count: p.entry_count,
          word_count: p.word_count,
        })),
      };
    }
    default:
      return { days: result };
  }
}

/** The part of `actual` that `expected` names: objects are compared key by key, the rest whole. */
function pick(actual: unknown, expected: unknown): unknown {
  if (typeof expected !== 'object' || expected === null || Array.isArray(expected)) return actual;
  const have = (actual ?? {}) as Record<string, unknown>;
  return Object.fromEntries(
    Object.keys(expected).map((k) => [k, pick(have[k], (expected as Record<string, unknown>)[k])]),
  );
}

function run(c: Case): unknown {
  const queue: number[] = [];
  let next = 0;
  const seeds = c.seed.flatMap((s) =>
    Array.from({ length: s.repeat ?? 1 }, () => {
      const n = s.id_n ?? next;
      next++;
      return { ...s, n };
    }),
  );
  vi.spyOn(crypto, 'randomUUID').mockImplementation(
    () => idOf(queue.shift() ?? 9999) as `${string}-${string}-${string}-${string}-${string}`,
  );
  const mock = createJournalMock();
  const index = new Map<string, number>();
  let last = 0;
  seeds.forEach((s, i) => {
    const at = s.at ?? i * 10;
    last = Math.max(last, at);
    vi.setSystemTime(START + at);
    queue.push(s.n);
    mock.handle('journal_entry_create', {
      entry: {
        page_date: s.page_date ?? '2026-10-20',
        text: s.body,
        written_tz: 'UTC',
        written_offset_min: 0,
        session_id: 's1',
      },
    });
    index.set(idOf(s.n), i);
  });
  vi.setSystemTime(START + last + (c.age_minutes ?? 0) * 60_000);

  const args = { ...c.args };
  if (typeof args.id === 'number') args.id = idOf(seeds[args.id]?.n ?? 0);
  try {
    return normalise(c.op, mock.handle(command(c.op), args), index);
  } catch (e) {
    const { code, message } = e as { code: string; message: string };
    return { error: { code, message } };
  }
}

describe('journal rules shared with the Rust commands', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'] });
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  for (const c of rules.cases as Case[]) {
    it(`${c.rule}: ${c.name}`, () => {
      expect(pick(run(c), c.expect)).toEqual(c.expect);
    });
  }
});
