import { describe, expect, it } from 'vitest';
import { localDateString, pageTitle, writtenTime } from './dates';

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

describe('pageTitle', () => {
  it('writes weekday, day and month', () => {
    expect(pageTitle('2026-10-20', '2026-10-21')).toBe('Tuesday, 20 October');
  });
  it('adds the year only for other years', () => {
    expect(pageTitle('2025-12-31', '2026-01-01')).toBe('Wednesday, 31 December 2025');
  });
});
