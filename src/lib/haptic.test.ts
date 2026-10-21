import { describe, expect, it, vi } from 'vitest';
import { lightTick, TICK_MS } from './haptic';

describe('lightTick', () => {
  it('vibrates briefly', () => {
    const vibrate = vi.fn(() => true);
    lightTick({ vibrate });
    expect(vibrate).toHaveBeenCalledWith(TICK_MS);
  });
  it('does nothing without the API, and swallows a refusal', () => {
    expect(() => lightTick({} as Navigator)).not.toThrow();
    expect(() =>
      lightTick({
        vibrate: () => {
          throw new Error('blocked');
        },
      }),
    ).not.toThrow();
  });
});
