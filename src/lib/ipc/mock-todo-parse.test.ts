import { describe, expect, it } from 'vitest';
import type { ParseRequest } from './bindings';
import fixtures from './fixtures/parse-cases.json';
import { foldName } from './mock-todo';
import { type ListRef, parseText } from './mock-todo-parse';

// The same cases run against the Rust parser (crates/todo/src/parse.rs, ADR 0009).
type Fields = Record<string, unknown>;
interface Case extends Fields {
  name: string;
  expect: Fields;
}

const brief = (spans: { word: string; kind: string; value: string }[]) =>
  spans.map((s) => [s.word, s.kind, s.value]);

describe('the shared parse fixtures', () => {
  for (const c of fixtures.cases as Case[]) {
    it(c.name, () => {
      const merged = { ...fixtures.defaults, ...c } as Fields & { lists: ListRef[] };
      const got = parseText(merged as unknown as ParseRequest, merged.lists, foldName);
      for (const [key, want] of Object.entries(c.expect)) {
        const have =
          key === 'spans'
            ? brief(got.spans)
            : key === 'ignored'
              ? brief(got.ignored)
              : key === 'alternatives'
                ? got.alternatives.map((a) => [a.word, a.kind, a.value, a.reason])
                : (got as unknown as Fields)[key];
        expect(have, key).toEqual(want);
      }
    });
  }
});

describe('the mock parser', () => {
  const read = (text: string) =>
    parseText(
      {
        text,
        ignored: [],
        prefer: {},
        frozen: [],
        today: '2026-10-21',
        now: '14:05',
        week_start: 'mon',
      },
      [{ id: 'l-home', name: 'Home' }],
      foldName,
    );

  it('reports where each word is in UTF-16 units', () => {
    const p = read('Party \u{1F389} tomorrow');
    expect([p.spans[0]?.start, p.spans[0]?.end]).toEqual([9, 17]);
  });

  it('keeps spans in order and apart', () => {
    const p = read('p2 every mon and thu 9am #ho next fri');
    let at = 0;
    for (const s of p.spans) {
      expect(s.start).toBeGreaterThanOrEqual(at);
      expect(s.end).toBeGreaterThan(s.start);
      at = s.end;
    }
  });

  it('numbers repeated words so each can be kept as text on its own', () => {
    const p = read('tomorrow and tomorrow');
    expect(p.alternatives[0]?.key).toBe('date:tomorrow:1');
    expect(p.spans[0]?.key).toBe('date:tomorrow:2');
  });
});
