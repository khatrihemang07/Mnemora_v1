import { describe, expect, it } from 'vitest';
import { formatHm, timeSuggestions } from './when-time';

describe('timeSuggestions', () => {
  it('offers nothing until something is typed', () => {
    expect(timeSuggestions('')).toEqual([]);
    expect(timeSuggestions('   ')).toEqual([]);
  });

  it('reads a bare hour as either half of the day', () => {
    expect(timeSuggestions('9')).toEqual(['09:00', '21:00']);
    expect(timeSuggestions('12')).toEqual(['12:00', '00:00']);
    expect(timeSuggestions('1')).toEqual(['01:00', '13:00']);
  });

  it('reads an hour that can only be one thing as that', () => {
    expect(timeSuggestions('13')).toEqual(['13:00']);
    expect(timeSuggestions('21')).toEqual(['21:00']);
    expect(timeSuggestions('0')).toEqual(['00:00']);
  });

  it('reads am and pm marks', () => {
    expect(timeSuggestions('930p')).toEqual(['21:30']);
    expect(timeSuggestions('9:30 PM')).toEqual(['21:30']);
    expect(timeSuggestions('9am')).toEqual(['09:00']);
    expect(timeSuggestions('12a')).toEqual(['00:00']);
    expect(timeSuggestions('12pm')).toEqual(['12:00']);
    expect(timeSuggestions('13pm')).toEqual([]);
  });

  it('reads a full 24-hour time as itself and the words noon and midnight', () => {
    expect(timeSuggestions('21:30')).toEqual(['21:30']);
    expect(timeSuggestions('2130')).toEqual(['21:30']);
    expect(timeSuggestions('00:05')).toEqual(['00:05']);
    expect(timeSuggestions('noon')).toEqual(['12:00']);
    expect(timeSuggestions(' Midnight ')).toEqual(['00:00']);
  });

  it('reads an hour and minutes with no mark as either half of the day when it can be', () => {
    expect(timeSuggestions('9:30')).toEqual(['09:30', '21:30']);
    expect(timeSuggestions('930')).toEqual(['09:30', '21:30']);
  });

  it('refuses what is not a time', () => {
    for (const bad of ['25', '9:60', '2 tickets', 'abc', '99:99', '9:5']) {
      expect(timeSuggestions(bad), bad).toEqual([]);
    }
  });
});

describe('formatHm', () => {
  it('follows the time format Setting', () => {
    expect(formatHm('21:30', '24h')).toBe('21:30');
    expect(formatHm('21:30', '12h')).toBe('9:30 PM');
    expect(formatHm('00:05', '12h')).toBe('12:05 AM');
    expect(formatHm('12:00', '12h')).toBe('12:00 PM');
    expect(formatHm('09:00', '12h')).toBe('9:00 AM');
  });
});
