import { describe, expect, it } from 'vitest';
import { slashValues } from './slash-values';

describe('slashValues', () => {
  const at = new Date(2026, 9, 20, 14, 5);
  it('uses the core time formats', () => {
    expect(slashValues(at, 'iso', '24h').time).toBe('14:05');
    expect(slashValues(at, 'iso', '12h').time).toBe('2:05 PM');
  });
  it('uses the core date formats', () => {
    expect(slashValues(at, 'iso', '24h').date).toBe('2026-10-20');
    expect(slashValues(at, 'long', '24h').date).toBe('Tuesday, October 20');
    expect(slashValues(at, 'short', '24h').date).toBe('Oct 20, 2026');
  });
});
