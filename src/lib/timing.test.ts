import { afterEach, describe, expect, it, vi } from 'vitest';
import { setTimer } from './timing';

describe('setTimer', () => {
  afterEach(() => vi.useRealTimers());

  it('runs the callback after the delay', () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    setTimer(fn, 100);
    vi.advanceTimersByTime(99);
    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(fn).toHaveBeenCalledOnce();
  });

  it('returns a function that cancels it', () => {
    vi.useFakeTimers();
    const fn = vi.fn();
    setTimer(fn, 100)();
    vi.advanceTimersByTime(200);
    expect(fn).not.toHaveBeenCalled();
  });
});
