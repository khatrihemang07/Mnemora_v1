import { describe, expect, it } from 'vitest';
import type { Parsed, ParseSpan } from '../../lib/ipc/bindings';
import { whenFromParse } from './when-typed';

const span = (
  word: string,
  kind: string,
  start: number,
  over: Partial<ParseSpan> = {},
): ParseSpan => ({
  key: `${kind}:${word}:1`,
  kind,
  start,
  end: start + word.length,
  word,
  value: '',
  new_list: false,
  ...over,
});

const parsed = (spans: ParseSpan[], over: Partial<Parsed> = {}): Parsed => ({
  spans,
  alternatives: [],
  ignored: [],
  date: null,
  time: null,
  repeat: null,
  priority: null,
  list_id: null,
  new_list: null,
  ...over,
});

describe('whenFromParse', () => {
  it('takes a date', () => {
    const p = parsed([span('fri', 'date', 0)], { date: '2026-10-23' });
    expect(whenFromParse('fri', p)).toEqual({ date: '2026-10-23', time: null });
  });

  it('takes a time alone without the day the parser guessed', () => {
    const p = parsed([span('9am', 'time', 0)], { date: '2026-10-22', time: '09:00' });
    expect(whenFromParse('9am', p)).toEqual({ date: null, time: '09:00' });
  });

  it('takes a date and a time, with spaces and a full stop around them', () => {
    const p = parsed([span('tomorrow', 'date', 0), span('9am', 'time', 9)], {
      date: '2026-10-22',
      time: '09:00',
    });
    expect(whenFromParse('tomorrow 9am.', p)).toEqual({ date: '2026-10-22', time: '09:00' });
  });

  it('takes "tonight", which is a date and a time in one word', () => {
    const p = parsed([span('tonight', 'date', 0), span('tonight', 'time', 0)], {
      date: '2026-10-21',
      time: '20:00',
    });
    expect(whenFromParse('tonight', p)).toEqual({ date: '2026-10-21', time: '20:00' });
  });

  it('is not a date when other words come with it', () => {
    const p = parsed([span('fri', 'date', 9)], { date: '2026-10-23' });
    expect(whenFromParse('buy milk fri', p)).toBeNull();
  });

  it('is not a date when it also holds a Repeat, a Priority or a List', () => {
    for (const kind of ['repeat', 'prio', 'list']) {
      const p = parsed([span('fri', 'date', 0), span('x', kind, 4)], { date: '2026-10-23' });
      expect(whenFromParse('fri x', p), kind).toBeNull();
    }
  });

  it('does not guess between two dates, or read nothing', () => {
    const p = parsed([span('today', 'date', 12)], {
      date: '2026-10-21',
      alternatives: [{ ...span('tomorrow', 'date', 0), reason: 'conflict' }],
    });
    expect(whenFromParse('tomorrow or today', p)).toBeNull();
    expect(whenFromParse('hello', parsed([]))).toBeNull();
    expect(whenFromParse('hello', null)).toBeNull();
  });
});
