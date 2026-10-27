import { describe, expect, it } from 'vitest';
import { returnFocus } from './focus';

/** A stand-in element that counts the times it was focused. */
function element(isConnected: boolean) {
  const el = {
    isConnected,
    focused: 0,
    focus() {
      el.focused += 1;
    },
  };
  return el;
}

describe('returnFocus', () => {
  it('gives focus back to the opener while it is still on screen', () => {
    const opener = element(true);
    const fallback = element(true);
    returnFocus(opener, fallback);
    expect(opener.focused).toBe(1);
    expect(fallback.focused).toBe(0);
  });

  it('falls back to the question box when the opener has gone', () => {
    const opener = element(false);
    const fallback = element(true);
    returnFocus(opener, fallback);
    expect(opener.focused).toBe(0);
    expect(fallback.focused).toBe(1);
  });

  it('does nothing when there is no opener and no fallback', () => {
    expect(() => returnFocus(null, null)).not.toThrow();
    expect(() => returnFocus(undefined)).not.toThrow();
  });
});
