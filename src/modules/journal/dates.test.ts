import { describe, expect, it } from 'vitest';
import {
  editedLabel,
  fullWritten,
  localDateString,
  monthLabel,
  pageHeading,
  pageSummary,
  showTodayPill,
  writtenDateLabel,
  writtenTime,
} from './dates';

describe('localDateString', () => {
  it('uses the local calendar and pads', () => {
    expect(localDateString(new Date(2026, 0, 5, 23, 59))).toBe('2026-01-05');
    expect(localDateString(new Date(2026, 9, 20, 0, 0))).toBe('2026-10-20');
  });
});

describe('writtenTime', () => {
  const at = Date.UTC(2026, 9, 20, 16, 30);
  it('shows the wall clock of the written zone, whatever the viewer zone', () => {
    expect(writtenTime(at, 330, '24h')).toBe('22:00'); // India
    expect(writtenTime(at, 60, '24h')).toBe('17:30');
    expect(writtenTime(at, -480, '24h')).toBe('08:30');
  });
  it('wraps past midnight and supports 12h', () => {
    expect(writtenTime(at, 450, '24h')).toBe('00:00');
    expect(writtenTime(at, 330, '12h')).toBe('10:00 PM');
    expect(writtenTime(at, 450, '12h')).toBe('12:00 AM');
  });
});

describe('pageHeading', () => {
  it('splits the date, the year and the badge', () => {
    expect(pageHeading('2026-10-20', '2026-10-20')).toEqual({
      title: 'Tuesday, 20 October',
      year: null,
      badge: 'Today',
    });
    expect(pageHeading('2026-10-19', '2026-10-20').badge).toBe('Yesterday');
    expect(pageHeading('2026-10-18', '2026-10-20').badge).toBeNull();
  });
  it('keeps the real date on a badged Page and finds Yesterday across a month or year', () => {
    expect(pageHeading('2026-09-30', '2026-10-01').badge).toBe('Yesterday');
    expect(pageHeading('2025-12-31', '2026-01-01')).toEqual({
      title: 'Wednesday, 31 December',
      year: '2025',
      badge: 'Yesterday',
    });
  });
  it('treats a future Page like any other date', () => {
    expect(pageHeading('2026-10-21', '2026-10-20').badge).toBeNull();
    expect(pageHeading('2027-01-02', '2026-10-20').year).toBe('2027');
  });
});

describe('writtenDateLabel', () => {
  const at = Date.UTC(2026, 9, 19, 3, 42); // 09:12 on Mon 19 Oct in India (+05:30)
  it('is null when written on the Page date, in the zone it was written', () => {
    expect(writtenDateLabel(at, 330, '2026-10-19')).toBeNull();
  });
  it('shows a short date when written on another day', () => {
    expect(writtenDateLabel(at, 330, '2026-10-16')).toBe('Mon 19 Oct');
  });
  it('uses the written zone, not the viewer zone, to find the day', () => {
    // 22:00 UTC on the 18th is already the 19th in India.
    const late = Date.UTC(2026, 9, 18, 22, 0);
    expect(writtenDateLabel(late, 330, '2026-10-19')).toBeNull();
    expect(writtenDateLabel(late, 0, '2026-10-19')).toBe('Sun 18 Oct');
  });
  it('adds the year when it differs from the Page year', () => {
    expect(writtenDateLabel(at, 330, '2025-12-30')).toBe('Mon 19 Oct 2026');
  });
});

describe('fullWritten', () => {
  const at = Date.UTC(2026, 9, 19, 3, 42);
  it('spells out the date, time and zone', () => {
    expect(fullWritten(at, 330, 'Asia/Kolkata', '24h')).toBe(
      'Written Monday, 19 October 2026 at 09:12 (Asia/Kolkata)',
    );
    expect(fullWritten(at, 330, 'Asia/Kolkata', '12h')).toBe(
      'Written Monday, 19 October 2026 at 9:12 AM (Asia/Kolkata)',
    );
  });
});

describe('editedLabel', () => {
  it('names the edit moment in the given offset', () => {
    const at = Date.UTC(2026, 9, 20, 8, 32);
    expect(editedLabel(at, 330, '24h')).toBe('Edited Tue 20 Oct, 14:02');
  });
});

describe('pageSummary', () => {
  it('handles singular and plural', () => {
    expect(pageSummary(1, 1)).toBe('1 entry · 1 word');
    expect(pageSummary(2, 40)).toBe('2 entries · 40 words');
    expect(pageSummary(1, 0)).toBe('1 entry · 0 words');
  });
});

describe('monthLabel', () => {
  it('is the uppercase-able month and year', () => {
    expect(monthLabel('2026-09-30')).toBe('September 2026');
  });
});

describe('showTodayPill', () => {
  it('appears after about a screen of scrolling', () => {
    expect(showTodayPill(0, 800)).toBe(false);
    expect(showTodayPill(800, 800)).toBe(false);
    expect(showTodayPill(801, 800)).toBe(true);
  });
});
