import { describe, expect, it } from 'vitest';
import type { ParseSpan } from '../../lib/ipc/bindings';
import {
  liveUnderstood,
  type MeaningContext,
  meaningText,
  mergeUnderstood,
  parseUnderstood,
  serializeUnderstood,
  spanMeaning,
  stillHolds,
  type Understood,
  type UnderstoodFields,
  understoodFromSpans,
  understoodMarks,
} from './understood';

const fields = (over: Partial<UnderstoodFields> = {}): UnderstoodFields => ({
  due_date: '2026-10-22',
  due_time: '09:00',
  repeat_freq: 'month',
  repeat_every: 1,
  repeat_days: 22,
  repeat_mode: 'fixed',
  priority: 1,
  list_id: 'l-home',
  ...over,
});

const REPEAT = '{"freq":"month","every":1,"days":null,"mode":"fixed"}';
const record: Understood[] = [
  { word: 'tomorrow', kind: 'date', value: '2026-10-22' },
  { word: '9am', kind: 'time', value: '09:00' },
  { word: 'every month', kind: 'repeat', value: REPEAT },
  { word: 'p1', kind: 'prio', value: '1' },
  { word: '#Home', kind: 'list', value: 'l-home' },
];

const cx: MeaningContext = {
  today: '2026-10-21',
  timeFormat: '24h',
  listName: (id) => (id === 'l-home' ? 'Home' : null),
};

describe('the stored record', () => {
  it('round-trips and treats anything unreadable as nothing', () => {
    expect(parseUnderstood(serializeUnderstood(record))).toEqual(record);
    for (const bad of [
      null,
      '',
      'nope',
      '{"a":1}',
      '[1,2]',
      '[{"word":"x","kind":"zzz","value":"1"}]',
    ]) {
      expect(parseUnderstood(bad)).toEqual([]);
    }
    expect(serializeUnderstood([])).toBeNull();
  });
});

describe('the drift rule', () => {
  it('holds while every field still has what the word set', () => {
    for (const u of record) expect(stillHolds(u, fields()), u.kind).toBe(true);
  });

  it('drops a word once its field is changed by hand', () => {
    const [date, time, repeat, prio, list] = record as [
      Understood,
      Understood,
      Understood,
      Understood,
      Understood,
    ];
    expect(stillHolds(date, fields({ due_date: '2026-10-23' }))).toBe(false);
    expect(stillHolds(time, fields({ due_time: null }))).toBe(false);
    const none = { repeat_freq: null, repeat_every: null, repeat_mode: null };
    expect(stillHolds(repeat, fields(none))).toBe(false);
    expect(stillHolds(repeat, fields({ repeat_every: 2 }))).toBe(false);
    expect(stillHolds(repeat, fields({ repeat_mode: 'after' }))).toBe(false);
    expect(stillHolds(prio, fields({ priority: 2 }))).toBe(false);
    expect(stillHolds(list, fields({ list_id: null }))).toBe(false);
  });

  it('leaves the other words alone when one field changes', () => {
    const live = liveUnderstood({
      ...fields({ priority: 3 }),
      understood: serializeUnderstood(record),
    });
    expect(live.map((u) => u.kind)).toEqual(['date', 'time', 'repeat', 'list']);
  });

  it('does not mind a month Repeat re-anchoring to the date, but a weekly one keeps its days', () => {
    const month = record[2] as Understood;
    expect(stillHolds(month, fields({ repeat_days: 28 }))).toBe(true);
    const weekly: Understood = {
      word: 'every mon',
      kind: 'repeat',
      value: '{"freq":"week","every":1,"days":2,"mode":"fixed"}',
    };
    const base = { repeat_freq: 'week', repeat_every: 1, repeat_mode: 'fixed' };
    expect(stillHolds(weekly, fields({ ...base, repeat_days: 2 }))).toBe(true);
    expect(stillHolds(weekly, fields({ ...base, repeat_days: 18 }))).toBe(false);
  });

  it('is never true for a Task with no record', () => {
    expect(liveUnderstood({ ...fields(), understood: null })).toEqual([]);
  });
});

describe('meanings', () => {
  it('says each kind in the user terms', () => {
    expect(meaningText('date', '2026-10-23', cx)).toBe('Fri 23 Oct');
    expect(meaningText('date', '2027-01-02', cx)).toBe('Sat 2 Jan 2027');
    expect(meaningText('time', '21:30', cx)).toBe('21:30');
    expect(meaningText('time', '21:30', { ...cx, timeFormat: '12h' })).toBe('9:30 PM');
    expect(meaningText('prio', '2', cx)).toBe('P2');
    expect(meaningText('repeat', REPEAT, cx)).toBe('Every month');
    expect(meaningText('list', 'l-home', cx)).toBe('Home');
    expect(meaningText('list', 'l-gone', cx)).toBe('a List that is gone');
  });

  it('says a parsed List by name and a new one as an offer', () => {
    const span = (over: Partial<ParseSpan>): ParseSpan => ({
      key: 'k',
      kind: 'list',
      start: 0,
      end: 1,
      word: '#x',
      value: 'Home',
      new_list: false,
      ...over,
    });
    expect(spanMeaning(span({}), cx)).toBe('Home');
    expect(spanMeaning(span({ value: 'Garage', new_list: true }), cx)).toBe('New list “Garage”');
    expect(spanMeaning(span({ kind: 'date', value: '2026-10-22' }), cx)).toBe('Thu 22 Oct');
  });

  it('builds the tooltip for each word still understood, joining words that share text', () => {
    const tonight: Understood[] = [
      { word: 'tonight', kind: 'date', value: '2026-10-21' },
      { word: 'tonight', kind: 'time', value: '20:00' },
    ];
    const marks = understoodMarks(
      {
        ...fields({ due_date: '2026-10-21', due_time: '20:00' }),
        understood: serializeUnderstood(tonight),
      },
      cx,
    );
    expect(marks).toEqual([{ word: 'tonight', title: 'Understood as Wed 21 Oct · 20:00' }]);
  });
});

describe('what a captured Task records', () => {
  const spans: ParseSpan[] = [
    {
      key: 'a',
      kind: 'date',
      start: 0,
      end: 8,
      word: 'tomorrow',
      value: '2026-10-22',
      new_list: false,
    },
    { key: 'b', kind: 'list', start: 9, end: 14, word: '#Home', value: 'Home', new_list: false },
    { key: 'c', kind: 'list', start: 15, end: 20, word: '#Zoo', value: 'Zoo', new_list: true },
  ];

  it('stores a List by its id, and skips one that has none', () => {
    const got = understoodFromSpans(spans, (s) => (s.value === 'Home' ? 'l-home' : null));
    expect(got).toEqual([
      { word: 'tomorrow', kind: 'date', value: '2026-10-22' },
      { word: '#Home', kind: 'list', value: 'l-home' },
    ]);
  });
});

describe('after a Quick edit', () => {
  const old: Understood[] = [
    { word: 'tomorrow', kind: 'date', value: '2026-10-22' },
    { word: 'p1', kind: 'prio', value: '1' },
  ];

  it('keeps old words that are still in the title and still hold', () => {
    expect(mergeUnderstood(old, [], 'Pay rent tomorrow p1', fields())).toEqual(old);
  });

  it('drops an old word that was deleted from the title', () => {
    expect(mergeUnderstood(old, [], 'Pay rent p1', fields())).toEqual([old[1]]);
  });

  it('drops an old word whose field the edit changed', () => {
    expect(mergeUnderstood(old, [], 'Pay rent tomorrow p1', fields({ priority: 2 }))).toEqual([
      old[0],
    ]);
  });

  it('lets a new word replace the old words of its kind', () => {
    const fresh: Understood[] = [{ word: 'fri', kind: 'date', value: '2026-10-23' }];
    const got = mergeUnderstood(
      old,
      fresh,
      'Pay rent tomorrow p1 fri',
      fields({ due_date: '2026-10-23' }),
    );
    expect(got).toEqual([old[1], fresh[0]]);
  });
});
