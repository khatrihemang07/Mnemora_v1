import { describe, expect, it } from 'vitest';
import { relativeTime } from './relative';

const now = new Date(2026, 9, 20, 12, 0, 0);
const ago = (ms: number) => new Date(now.getTime() - ms);
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

describe('relativeTime (N15-A)', () => {
  it.each([
    [10_000, 'just now'],
    [MIN, '1 min ago'],
    [59 * MIN, '59 min ago'],
    [HOUR, '1 h ago'],
    [23 * HOUR, '23 h ago'],
    [DAY, 'yesterday'],
    [3 * DAY, '3 days ago'],
    [6 * DAY + 23 * HOUR, '6 days ago'],
  ])('%i ms ago reads %s', (ms, text) => {
    expect(relativeTime(ago(ms), now)).toBe(text);
  });

  it('switches to an absolute date at 7 days', () => {
    expect(relativeTime(ago(7 * DAY), now)).toBe('Oct 13, 2026');
  });

  it('treats the future as just now', () => {
    expect(relativeTime(new Date(now.getTime() + MIN), now)).toBe('just now');
  });
});
