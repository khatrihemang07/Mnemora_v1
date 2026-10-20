import { describe, expect, it } from 'vitest';
import {
  addDays,
  EMPTY_RANGE,
  inRange,
  monthGrid,
  moveFocus,
  pickRange,
  shiftMonth,
  weekdayLabels,
} from './calendar';

describe('monthGrid', () => {
  it('is six weeks starting on Monday, padded with the neighbouring months', () => {
    // 1 October 2026 is a Thursday.
    const grid = monthGrid('2026-10', 'mon');
    expect(grid).toHaveLength(42);
    expect(grid[0]).toEqual({ date: '2026-09-28', day: 28, inMonth: false });
    expect(grid[3]).toEqual({ date: '2026-10-01', day: 1, inMonth: true });
    expect(grid.filter((d) => d.inMonth)).toHaveLength(31);
    expect(grid[41]?.date).toBe('2026-11-08');
  });

  it('starts on Sunday when asked', () => {
    const grid = monthGrid('2026-10', 'sun');
    expect(grid[0]?.date).toBe('2026-09-27');
    expect(grid[4]?.date).toBe('2026-10-01');
  });

  it('starts at the 1st when the month begins on the first weekday', () => {
    // 1 June 2026 is a Monday; 1 February 2026 is a Sunday.
    expect(monthGrid('2026-06', 'mon')[0]?.date).toBe('2026-06-01');
    expect(monthGrid('2026-02', 'sun')[0]?.date).toBe('2026-02-01');
    expect(monthGrid('2026-02', 'mon')[0]?.date).toBe('2026-01-26');
  });

  it('handles leap Februaries and year boundaries', () => {
    expect(monthGrid('2024-02', 'mon').filter((d) => d.inMonth)).toHaveLength(29);
    expect(monthGrid('2026-02', 'mon').filter((d) => d.inMonth)).toHaveLength(28);
    expect(monthGrid('2027-01', 'mon')[0]?.date).toBe('2026-12-28');
  });
});

describe('weekdayLabels', () => {
  it('rotates with the week start', () => {
    expect(weekdayLabels('mon')).toEqual(['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su']);
    expect(weekdayLabels('sun')).toEqual(['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa']);
  });
});

describe('shiftMonth and addDays', () => {
  it('cross year boundaries both ways', () => {
    expect(shiftMonth('2026-12', 1)).toBe('2027-01');
    expect(shiftMonth('2026-01', -1)).toBe('2025-12');
    expect(shiftMonth('2026-10', -22)).toBe('2024-12');
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
    expect(addDays('2024-03-01', -1)).toBe('2024-02-29');
  });
});

describe('moveFocus', () => {
  it('moves by day and week', () => {
    expect(moveFocus('2026-10-20', 'ArrowLeft', 'mon')).toBe('2026-10-19');
    expect(moveFocus('2026-10-20', 'ArrowRight', 'mon')).toBe('2026-10-21');
    expect(moveFocus('2026-10-20', 'ArrowUp', 'mon')).toBe('2026-10-13');
    expect(moveFocus('2026-10-31', 'ArrowDown', 'mon')).toBe('2026-11-07');
  });

  it('Home and End go to the ends of the week for the week start', () => {
    // 20 October 2026 is a Tuesday.
    expect(moveFocus('2026-10-20', 'Home', 'mon')).toBe('2026-10-19');
    expect(moveFocus('2026-10-20', 'End', 'mon')).toBe('2026-10-25');
    expect(moveFocus('2026-10-20', 'Home', 'sun')).toBe('2026-10-18');
    expect(moveFocus('2026-10-20', 'End', 'sun')).toBe('2026-10-24');
  });

  it('PageUp and PageDown keep the day, clamped to the month length', () => {
    expect(moveFocus('2026-03-31', 'PageUp', 'mon')).toBe('2026-02-28');
    expect(moveFocus('2026-01-31', 'PageDown', 'mon')).toBe('2026-02-28');
    expect(moveFocus('2026-12-15', 'PageDown', 'mon')).toBe('2027-01-15');
  });

  it('ignores other keys', () => {
    expect(moveFocus('2026-10-20', 'a', 'mon')).toBeNull();
  });
});

describe('range selection', () => {
  it('the first pick starts a range, the second completes it', () => {
    const one = pickRange(EMPTY_RANGE, '2026-10-10');
    expect(one).toEqual({ from: '2026-10-10', to: null });
    expect(pickRange(one, '2026-10-15')).toEqual({ from: '2026-10-10', to: '2026-10-15' });
  });

  it('an earlier second pick swaps the ends; the same day is a one-day range', () => {
    const one = pickRange(EMPTY_RANGE, '2026-10-10');
    expect(pickRange(one, '2026-10-02')).toEqual({ from: '2026-10-02', to: '2026-10-10' });
    expect(pickRange(one, '2026-10-10')).toEqual({ from: '2026-10-10', to: '2026-10-10' });
  });

  it('a pick after a complete range starts over', () => {
    const done = { from: '2026-10-02', to: '2026-10-10' };
    expect(pickRange(done, '2026-10-20')).toEqual({ from: '2026-10-20', to: null });
  });

  it('inRange includes both ends and treats a half-chosen range as its first day', () => {
    const r = { from: '2026-10-02', to: '2026-10-04' };
    expect(
      ['2026-10-01', '2026-10-02', '2026-10-03', '2026-10-04', '2026-10-05'].map((d) =>
        inRange(r, d),
      ),
    ).toEqual([false, true, true, true, false]);
    expect(inRange({ from: '2026-10-02', to: null }, '2026-10-02')).toBe(true);
    expect(inRange({ from: '2026-10-02', to: null }, '2026-10-03')).toBe(false);
    expect(inRange(EMPTY_RANGE, '2026-10-03')).toBe(false);
  });
});
