import { describe, expect, it } from 'vitest';
import { localContext } from './local-context';

describe('localContext', () => {
  it('gives the local date, weekday, time zone and week start', () => {
    const now = new Date(2026, 9, 25, 23, 40);
    expect(localContext(now, 'mon', 'Europe/London')).toEqual({
      today: '2026-10-25',
      weekday: 'Sunday',
      time_zone: 'Europe/London',
      week_start: 'mon',
    });
  });

  it('pads the month and day', () => {
    expect(localContext(new Date(2026, 0, 4), 'sun', 'UTC').today).toBe('2026-01-04');
  });
});
