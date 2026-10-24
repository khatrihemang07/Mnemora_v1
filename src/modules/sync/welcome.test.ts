import { describe, expect, it } from 'vitest';
import { showWelcome } from './welcome';

describe('when the Devices tab is the welcome card', () => {
  it('shows it with no paired device, whichever list noticed first (DV9)', () => {
    expect(showWelcome(0, undefined)).toBe(true);
    expect(showWelcome(undefined, 0)).toBe(true);
    expect(showWelcome(1, 0)).toBe(true);
    expect(showWelcome(0, 1)).toBe(true);
  });

  it('shows the paired devices otherwise, and nothing yet while unknown', () => {
    expect(showWelcome(2, 2)).toBe(false);
    expect(showWelcome(undefined, undefined)).toBe(false);
  });
});
