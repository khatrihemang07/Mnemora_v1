import { describe, expect, it } from 'vitest';
import {
  buildGrid,
  daysBetween,
  dayText,
  pageForDate,
  quickDays,
  weekdayOf,
  weekStartOf,
} from './when-grid';

// 21 October 2026 is a Wednesday.
const TODAY = '2026-10-21';

describe('weekStartOf', () => {
  it('is the Monday or the Sunday that starts the week', () => {
    expect(weekStartOf(TODAY, 'mon')).toBe('2026-10-19');
    expect(weekStartOf(TODAY, 'sun')).toBe('2026-10-18');
  });

  it('is the day itself when it starts the week, and handles Sunday on a Monday week', () => {
    expect(weekStartOf('2026-10-19', 'mon')).toBe('2026-10-19');
    expect(weekStartOf('2026-10-18', 'mon')).toBe('2026-10-12');
    expect(weekStartOf('2026-10-18', 'sun')).toBe('2026-10-18');
  });
});

describe('buildGrid', () => {
  it('is five weeks that start at this week, with no earlier week', () => {
    const grid = buildGrid(TODAY, 'mon', 0);
    expect(grid.cells).toHaveLength(35);
    expect(grid.start).toBe('2026-10-19');
    expect(grid.end).toBe('2026-11-22');
    expect(grid.cells[0]?.date).toBe('2026-10-19');
    expect(grid.canGoBack).toBe(false);
  });

  it('starts on Sunday with that Preference', () => {
    const grid = buildGrid(TODAY, 'sun', 0);
    expect(grid.start).toBe('2026-10-18');
    expect(grid.weekdays).toEqual(['S', 'M', 'T', 'W', 'T', 'F', 'S']);
    expect(buildGrid(TODAY, 'mon', 0).weekdays).toEqual(['M', 'T', 'W', 'T', 'F', 'S', 'S']);
  });

  it('disables the days before today and rings today', () => {
    const grid = buildGrid(TODAY, 'mon', 0);
    const flags = grid.cells.slice(0, 4).map((c) => [c.date, c.past, c.today]);
    expect(flags).toEqual([
      ['2026-10-19', true, false],
      ['2026-10-20', true, false],
      ['2026-10-21', false, true],
      ['2026-10-22', false, false],
    ]);
    expect(grid.cells.filter((c) => c.past)).toHaveLength(2);
  });

  it('puts a tiny month label on the 1st only', () => {
    const grid = buildGrid(TODAY, 'mon', 0);
    const labelled = grid.cells.filter((c) => c.monthLabel);
    expect(labelled.map((c) => [c.date, c.monthLabel])).toEqual([['2026-11-01', 'Nov']]);
  });

  it('pages by five weeks and never before this week', () => {
    const next = buildGrid(TODAY, 'mon', 1);
    expect(next.start).toBe('2026-11-23');
    expect(next.canGoBack).toBe(true);
    expect(next.cells.some((c) => c.past)).toBe(false);
    expect(buildGrid(TODAY, 'mon', -3).start).toBe('2026-10-19');
    expect(buildGrid(TODAY, 'mon', 2).start).toBe('2026-12-28');
  });

  it('names the month, the two months, or the two years it spans', () => {
    expect(buildGrid('2026-10-05', 'mon', 0).heading).toBe('Oct – Nov 2026');
    expect(buildGrid('2026-11-02', 'mon', 0).heading).toBe('Nov – Dec 2026');
    expect(buildGrid('2026-12-14', 'mon', 0).heading).toBe('Dec 2026 – Jan 2027');
    expect(buildGrid('2027-02-01', 'mon', 0).heading).toBe('Feb – Mar 2027');
  });

  it('labels each 1st it passes, across the year end', () => {
    const grid = buildGrid('2026-12-14', 'mon', 0);
    expect(grid.cells.filter((c) => c.monthLabel).map((c) => c.monthLabel)).toEqual(['Jan']);
  });
});

describe('pageForDate', () => {
  it('finds the page that holds a date', () => {
    expect(pageForDate(TODAY, TODAY, 'mon')).toBe(0);
    expect(pageForDate('2026-11-22', TODAY, 'mon')).toBe(0);
    expect(pageForDate('2026-11-23', TODAY, 'mon')).toBe(1);
    expect(pageForDate('2026-12-28', TODAY, 'mon')).toBe(2);
  });

  it('is the first page for earlier days', () => {
    expect(pageForDate('2026-10-01', TODAY, 'mon')).toBe(0);
  });
});

describe('quickDays', () => {
  const keys = (today: string, ws: 'mon' | 'sun' = 'mon') =>
    Object.fromEntries(quickDays(today, ws).map((d) => [d.key, d.date]));

  it('gives today, tomorrow, the coming Saturday and next Monday on a Wednesday', () => {
    expect(keys(TODAY)).toEqual({
      today: '2026-10-21',
      tomorrow: '2026-10-22',
      weekend: '2026-10-24',
      next_week: '2026-10-26',
    });
    expect(quickDays(TODAY, 'mon').map((d) => d.hint)).toEqual(['Wed', 'Thu', 'Sat', 'Mon']);
  });

  it('makes Weekend today on a Saturday or a Sunday', () => {
    expect(keys('2026-10-24').weekend).toBe('2026-10-24');
    expect(keys('2026-10-25').weekend).toBe('2026-10-25');
    expect(keys('2026-10-23').weekend).toBe('2026-10-24');
    expect(keys('2026-10-19').weekend).toBe('2026-10-24');
  });

  it('makes Next week the first day of next week from the Week start Preference', () => {
    expect(keys(TODAY, 'sun').next_week).toBe('2026-10-25');
    expect(quickDays(TODAY, 'sun')[3]?.hint).toBe('Sun');
    // On the last day of a Monday week, next week starts tomorrow.
    expect(keys('2026-10-25', 'mon').next_week).toBe('2026-10-26');
    expect(keys('2026-10-25', 'sun').next_week).toBe('2026-11-01');
  });
});

describe('dates', () => {
  it('counts days between and names weekdays', () => {
    expect(daysBetween('2026-10-21', '2026-10-24')).toBe(3);
    expect(daysBetween('2026-10-21', '2026-10-20')).toBe(-1);
    expect(daysBetween('2026-12-31', '2027-01-01')).toBe(1);
    expect(weekdayOf(TODAY)).toBe(3);
  });

  it('writes a day with the year only when it is not this year', () => {
    expect(dayText('2026-10-23', TODAY)).toBe('Fri 23 Oct');
    expect(dayText('2027-01-05', TODAY)).toBe('Tue 5 Jan 2027');
  });
});
