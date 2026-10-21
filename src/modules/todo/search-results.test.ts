import { describe, expect, it } from 'vitest';
import type { Span, Task, TaskHit } from '../../lib/ipc/bindings';
import { canSearch, litWords, resultsLabel, spansText, splitHits } from './search-results';

const hit = (title: string, state: Task['state']): TaskHit =>
  ({ task: { title, state } as Task, title: [], snippet: [], snippet_source: null }) as TaskHit;

describe('splitHits', () => {
  it('puts open Tasks first and Done and Won’t do after, keeping each group’s order', () => {
    const { open, done } = splitHits([
      hit('a', 'open'),
      hit('b', 'wont'),
      hit('c', 'open'),
      hit('d', 'done'),
    ]);
    expect(open.map((h) => h.task.title)).toEqual(['a', 'c']);
    expect(done.map((h) => h.task.title)).toEqual(['b', 'd']);
  });
});

describe('litWords', () => {
  it('folds the matched words and skips the plain ones', () => {
    const spans: Span[] = [
      { text: 'Buy ', matched: false },
      { text: 'Café', matched: true },
      { text: ' beans', matched: false },
    ];
    expect([...litWords(spans)]).toEqual(['cafe']);
    expect(spansText(spans)).toBe('Buy Café beans');
  });
});

describe('canSearch', () => {
  it('needs a letter or digit', () => {
    expect(canSearch('cafe')).toBe(true);
    expect(canSearch('7')).toBe(true);
    expect(canSearch('  ')).toBe(false);
    expect(canSearch('"*-')).toBe(false);
    expect(canSearch('')).toBe(false);
  });
});

describe('resultsLabel', () => {
  it('is singular for one', () => {
    expect(resultsLabel(1)).toBe('1 result');
    expect(resultsLabel(0)).toBe('0 results');
    expect(resultsLabel(12)).toBe('12 results');
  });
});
