import { describe, expect, it } from 'vitest';
import {
  type FeedEntryRef,
  type FocusState,
  focusReducer,
  NO_FOCUS,
  newestEntryId,
} from './focus-nav';

const e = (id: string, date: string): FeedEntryRef => ({ id, date });
// Feed order: newest Page first, oldest Entry first within a Page.
const entries = [
  e('f1', '2026-10-21'),
  e('t1', '2026-10-20'),
  e('t2', '2026-10-20'),
  e('y1', '2026-10-19'),
  e('o1', '2026-10-01'),
];
const on = (id: string): FocusState => ({ id, wantsMore: false });

describe('↑ and ↓', () => {
  it('walk the Entries across Pages in feed order', () => {
    let s = on('t1');
    s = focusReducer(s, { type: 'down' }, entries);
    expect(s.id).toBe('t2');
    s = focusReducer(s, { type: 'down' }, entries);
    expect(s.id).toBe('y1');
    s = focusReducer(s, { type: 'up' }, entries);
    s = focusReducer(s, { type: 'up' }, entries);
    expect(s.id).toBe('t1');
  });
  it('stop at the top', () => {
    expect(focusReducer(on('f1'), { type: 'up' }, entries)).toEqual(on('f1'));
  });
  it('ask for older Pages at the end, and move on once they are loaded', () => {
    const atEnd = focusReducer(on('o1'), { type: 'down' }, entries);
    expect(atEnd).toEqual({ id: 'o1', wantsMore: true });
    const more = [...entries, e('o0', '2026-09-30')];
    expect(focusReducer(atEnd, { type: 'down' }, more)).toEqual(on('o0'));
  });
  it('do nothing while nothing is focused', () => {
    expect(focusReducer(NO_FOCUS, { type: 'down' }, entries)).toEqual(NO_FOCUS);
    expect(focusReducer(NO_FOCUS, { type: 'up' }, entries)).toEqual(NO_FOCUS);
  });
});

describe('Enter and Esc', () => {
  it('keep the Entry focused through editing', () => {
    let s = on('t2');
    s = focusReducer(s, { type: 'edit', id: 't2' }, entries);
    expect(s.id).toBe('t2');
    s = focusReducer(s, { type: 'stop-edit' }, entries);
    expect(s.id).toBe('t2');
    expect(focusReducer(s, { type: 'down' }, entries).id).toBe('y1');
  });
  it('a click that opens an editor takes the focus there', () => {
    expect(focusReducer(on('t1'), { type: 'edit', id: 'y1' }, entries).id).toBe('y1');
  });
});

describe('from the composer', () => {
  it('lands on the last Entry of Today, not a future Page', () => {
    expect(focusReducer(NO_FOCUS, { type: 'from-composer', today: '2026-10-20' }, entries).id).toBe(
      't2',
    );
  });
  it('falls back to the newest earlier Entry when Today is empty', () => {
    const noToday = entries.filter((x) => x.date !== '2026-10-20');
    expect(newestEntryId(noToday, '2026-10-20')).toBe('y1');
  });
  it('does nothing when only future Pages are loaded, or none', () => {
    expect(newestEntryId([e('f1', '2026-10-21')], '2026-10-20')).toBeNull();
    expect(newestEntryId([], '2026-10-20')).toBeNull();
    expect(focusReducer(NO_FOCUS, { type: 'from-composer', today: '2026-10-20' }, [])).toEqual(
      NO_FOCUS,
    );
  });
});

describe('deleting', () => {
  it('moves focus to the next Entry, or the previous one at the end', () => {
    expect(focusReducer(on('t1'), { type: 'deleted', id: 't1' }, entries).id).toBe('t2');
    expect(focusReducer(on('o1'), { type: 'deleted', id: 'o1' }, entries).id).toBe('y1');
  });
  it('leaves nothing focused when it was the only Entry', () => {
    expect(focusReducer(on('a'), { type: 'deleted', id: 'a' }, [e('a', 'x')])).toEqual(NO_FOCUS);
  });
  it('ignores the deletion of an Entry that is not focused', () => {
    expect(focusReducer(on('t1'), { type: 'deleted', id: 'y1' }, entries)).toEqual(on('t1'));
  });
});

describe('focus and clear', () => {
  it('follow where focus lands and where it leaves', () => {
    expect(focusReducer(NO_FOCUS, { type: 'focus', id: 'y1' }, entries).id).toBe('y1');
    expect(focusReducer(on('y1'), { type: 'clear' }, entries)).toEqual(NO_FOCUS);
  });
});
