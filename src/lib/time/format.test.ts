import { describe, expect, it } from 'vitest';
import { clockParts, formatDate } from './format';

const d = new Date(2026, 9, 20, 15, 4, 9);

describe('clockParts', () => {
  it('24h pads the hour and has no period', () => {
    expect(clockParts(new Date(2026, 9, 20, 7, 4, 9), '24h')).toEqual({
      hm: '07:04',
      seconds: '09',
      period: null,
    });
  });
  it('12h uses a period and no padded hour', () => {
    expect(clockParts(d, '12h')).toEqual({ hm: '3:04', seconds: '09', period: 'PM' });
  });
  it('12h shows midnight and noon as 12', () => {
    expect(clockParts(new Date(2026, 9, 20, 0, 0, 0), '12h').hm).toBe('12:00');
    expect(clockParts(new Date(2026, 9, 20, 12, 30, 0), '12h')).toMatchObject({
      hm: '12:30',
      period: 'PM',
    });
  });
});

describe('formatDate', () => {
  it('long', () => expect(formatDate(d, 'long')).toBe('Tuesday, October 20'));
  it('short', () => expect(formatDate(d, 'short')).toBe('Oct 20, 2026'));
  it('iso uses the local date', () => expect(formatDate(d, 'iso')).toBe('2026-10-20'));
});
