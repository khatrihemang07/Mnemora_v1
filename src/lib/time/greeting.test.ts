import { describe, expect, it } from 'vitest';
import { greetingFor, greetingText } from './greeting';

const at = (h: number, m: number) => new Date(2026, 9, 20, h, m);

describe('greetingFor', () => {
  it.each([
    [4, 59, 'night'],
    [5, 0, 'morning'],
    [11, 59, 'morning'],
    [12, 0, 'afternoon'],
    [16, 59, 'afternoon'],
    [17, 0, 'evening'],
    [21, 59, 'evening'],
    [22, 0, 'night'],
    [0, 0, 'night'],
  ])('%i:%i is %s', (h, m, expected) => {
    expect(greetingFor(at(h, m))).toBe(expected);
  });
});

describe('greetingText', () => {
  it('adds the name when there is one', () => {
    expect(greetingText(at(9, 0), 'Hemang')).toBe('Good morning, Hemang');
  });
  it('drops the name when blank', () => {
    expect(greetingText(at(23, 0), '  ')).toBe('Good night');
  });
});
