import { describe, expect, it } from 'vitest';
import type { Entry } from './bindings';
import { searchEntries } from './mock-journal-search';

let n = 0;
const entry = (page_date: string, body: string): Entry => ({
  id: `e${n++}`,
  page_date,
  body,
  written_at: n,
  written_offset_min: 0,
  written_tz: 'UTC',
  edited_at: null,
  word_count: 1,
  created_at: n,
  updated_at: n,
});

const bodies = (hits: { id: string }[], all: Entry[]) =>
  hits.map((h) => all.find((e) => e.id === h.id)?.body);

describe('mock journal search', () => {
  const all = [
    entry('2026-10-01', 'Coffee at the Café'),
    entry('2026-10-20', 'my journal today'),
    entry('2026-10-19', 'journals everywhere'),
    entry('2026-10-10', 'the cat sat'),
  ];

  it('ignores case and accents, matches prefixes and needs every word', () => {
    expect(bodies(searchEntries(all, 'CAFE', null, null).exact, all)).toEqual([
      'Coffee at the Café',
    ]);
    expect(searchEntries(all, 'jour', null, null).exact).toHaveLength(2);
    expect(searchEntries(all, 'today journal', null, null).exact).toHaveLength(1);
    expect(searchEntries(all, 'journal zebra', null, null).exact).toHaveLength(0);
  });

  it('lists newest Page first', () => {
    expect(bodies(searchEntries(all, 'jour', null, null).exact, all)).toEqual([
      'my journal today',
      'journals everywhere',
    ]);
  });

  it('keeps typos in a separate group, by word length, never for short words', () => {
    const r = searchEntries(all, 'jurnal', null, null);
    expect(r.exact).toHaveLength(0);
    expect(bodies(r.similar, all)).toEqual(['my journal today']);
    expect(searchEntries(all, 'cot', null, null).similar).toHaveLength(0);
    expect(searchEntries(all, 'journal', null, null).similar).toHaveLength(0);
  });

  it('marks matches in the snippet', () => {
    const [hit] = searchEntries(all, 'cafe', null, null).exact;
    expect(hit?.snippet.filter((s) => s.matched).map((s) => s.text)).toEqual(['Café']);
  });

  it('phrases are whole and ordered', () => {
    const entries = [entry('2026-10-01', 'the red door'), entry('2026-10-02', 'door red')];
    expect(searchEntries(entries, '"red door"', null, null).exact).toHaveLength(1);
  });

  it('a range alone lists every Entry in it; nothing at all lists none', () => {
    expect(searchEntries(all, '', '2026-10-10', '2026-10-19').exact).toHaveLength(2);
    expect(searchEntries(all, '  ', null, null).exact).toHaveLength(0);
    expect(searchEntries(all, 'jour', '2026-10-20', null).exact).toHaveLength(1);
  });
});
