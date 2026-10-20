import { describe, expect, it } from 'vitest';
import type { TodaySummary } from '../../lib/ipc/bindings';
import { todayCardText } from './today-card';

const latest = (snippet: string) => ({
  id: 'a',
  written_at: 1,
  written_offset_min: 0,
  written_tz: 'UTC',
  snippet,
});

describe('todayCardText', () => {
  it('quotes the latest snippet and counts the entries', () => {
    const s: TodaySummary = { count: 3, latest: latest('A quiet morning') };
    expect(todayCardText(s)).toEqual({ quote: '“A quiet morning”', meta: 'Today · 3 entries' });
  });
  it('says "1 entry" in the singular', () => {
    expect(todayCardText({ count: 1, latest: latest('x') }).meta).toBe('Today · 1 entry');
  });
  it('says nothing was written on an empty day', () => {
    expect(todayCardText({ count: 0, latest: null })).toEqual({
      quote: null,
      meta: 'Nothing written today',
    });
  });
});
