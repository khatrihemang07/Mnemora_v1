import { describe, expect, it } from 'vitest';
import { simpleWhenParser, type WhenContext } from './when-parse';

// 21 October 2026 is a Wednesday.
const ctx: WhenContext = { today: '2026-10-21', now: '14:05', weekStart: 'mon' };
const read = (text: string, c: WhenContext = ctx) => simpleWhenParser(text, c);
const date = (text: string, c: WhenContext = ctx) => read(text, c)?.date;

describe('simpleWhenParser', () => {
  it('reads today, tomorrow and tonight', () => {
    expect(date('today')).toBe('2026-10-21');
    expect(date(' Tomorrow ')).toBe('2026-10-22');
    expect(date('tmr')).toBe('2026-10-22');
    expect(read('tonight')).toEqual({ date: '2026-10-21', time: '20:00' });
  });

  it('reads a weekday as the next one, today when it is that day', () => {
    expect(date('fri')).toBe('2026-10-23');
    expect(date('on friday')).toBe('2026-10-23');
    expect(date('wed')).toBe('2026-10-21');
    expect(date('tue')).toBe('2026-10-27');
    expect(date('sun')).toBe('2026-10-25');
  });

  it('reads next weekday as that day of next week, from the Week start Preference', () => {
    expect(date('next fri')).toBe('2026-10-30');
    expect(date('next mon')).toBe('2026-10-26');
    expect(date('next sun')).toBe('2026-11-01');
    expect(date('next sun', { ...ctx, weekStart: 'sun' })).toBe('2026-10-25');
  });

  it('reads weekend and next week like the quick days', () => {
    expect(date('weekend')).toBe('2026-10-24');
    expect(date('this weekend')).toBe('2026-10-24');
    expect(date('next week')).toBe('2026-10-26');
    expect(date('next week', { ...ctx, weekStart: 'sun' })).toBe('2026-10-25');
  });

  it('reads in N days or weeks', () => {
    expect(date('in 3 days')).toBe('2026-10-24');
    expect(date('in 1 day')).toBe('2026-10-22');
    expect(date('in 2 weeks')).toBe('2026-11-04');
    expect(date('in 10d')).toBe('2026-10-31');
  });

  it('reads a day and month in either order', () => {
    expect(date('28 oct')).toBe('2026-10-28');
    expect(date('oct 28')).toBe('2026-10-28');
    expect(date('3rd nov')).toBe('2026-11-03');
    expect(date('1 January')).toBe('2027-01-01');
  });

  it('reads a past day and month as next year, and refuses a past year', () => {
    expect(date('18 oct')).toBe('2027-10-18');
    expect(date('21 oct')).toBe('2026-10-21');
    expect(date('1 jan 2025')).toBeUndefined();
    expect(date('1 jan 2028')).toBe('2028-01-01');
    expect(date('29 feb')).toBe('2028-02-29');
  });

  it('reads an ISO date that is not in the past', () => {
    expect(date('2026-12-25')).toBe('2026-12-25');
    expect(read('2026-10-20')).toBeNull();
    expect(read('2026-02-30')).toBeNull();
  });

  it('reads a time on its own, after the date or before it', () => {
    expect(read('9am')).toEqual({ date: null, time: '09:00' });
    expect(read('at 9:30pm')).toEqual({ date: null, time: '21:30' });
    expect(read('noon')).toEqual({ date: null, time: '12:00' });
    expect(read('fri 9am')).toEqual({ date: '2026-10-23', time: '09:00' });
    expect(read('tomorrow at 21:30')).toEqual({ date: '2026-10-22', time: '21:30' });
    expect(read('9am tomorrow')).toEqual({ date: '2026-10-22', time: '09:00' });
    expect(read('28 oct 5pm')).toEqual({ date: '2026-10-28', time: '17:00' });
  });

  it('never reads numeric dates or a lone number, and says nothing for the rest', () => {
    for (const text of ['2/3', '9', '', '  ', 'banana', 'fri banana', 'in 3', '32 oct', 'oct']) {
      expect(read(text), text).toBeNull();
    }
  });
});
