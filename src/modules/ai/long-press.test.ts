import { describe, expect, it } from 'vitest';
import { LONG_PRESS_MS, pressMovedTooFar } from './long-press';

describe('the long press', () => {
  it('holds for half a second', () => {
    expect(LONG_PRESS_MS).toBe(500);
  });

  it('is still a long press while the finger stays within a few pixels', () => {
    expect(pressMovedTooFar({ x: 0, y: 0 }, { x: 3, y: 4 })).toBe(false);
  });

  it('is cancelled once the finger has moved on, as a scroll does', () => {
    expect(pressMovedTooFar({ x: 0, y: 0 }, { x: 30, y: 0 })).toBe(true);
  });
});
