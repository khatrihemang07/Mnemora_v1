import { describe, expect, it } from 'vitest';
import { holdMulticastWhileVisible } from './multicast';

function fakeDocument(hidden = false) {
  const listeners = new Set<() => void>();
  return {
    hidden,
    addEventListener: (_: 'visibilitychange', fn: () => void) => void listeners.add(fn),
    removeEventListener: (_: 'visibilitychange', fn: () => void) => void listeners.delete(fn),
    fire() {
      for (const fn of listeners) fn();
    },
    count: () => listeners.size,
  };
}

describe('holdMulticastWhileVisible', () => {
  it('does nothing without the Android bridge', () => {
    const doc = fakeDocument();
    const stop = holdMulticastWhileVisible(undefined, doc);
    expect(doc.count()).toBe(0);
    stop();
  });

  it('holds while visible, lets go when hidden, and releases on stop', () => {
    const doc = fakeDocument();
    const calls: boolean[] = [];
    const stop = holdMulticastWhileVisible({ holdMulticastLock: (h) => calls.push(h) }, doc);
    expect(calls).toEqual([true]);
    doc.hidden = true;
    doc.fire();
    doc.hidden = false;
    doc.fire();
    expect(calls).toEqual([true, false, true]);
    stop();
    expect(calls).toEqual([true, false, true, false]);
    expect(doc.count()).toBe(0);
  });
});
