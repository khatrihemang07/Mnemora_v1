import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEVICES_REFRESH_MS, refreshWhileVisible } from './devices-refresh';

const doc = new EventTarget() as EventTarget & { visibilityState: string };
const win = new EventTarget();

describe('refreshing this device while the Devices page is visible', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    doc.visibilityState = 'visible';
  });
  afterEach(() => vi.useRealTimers());

  const start = (refresh: () => void) =>
    refreshWhileVisible(refresh, DEVICES_REFRESH_MS, doc as never, win as never);

  it('refreshes on every interval', () => {
    const refresh = vi.fn();
    const stop = start(refresh);
    vi.advanceTimersByTime(DEVICES_REFRESH_MS * 2);
    expect(refresh).toHaveBeenCalledTimes(2);
    stop();
  });

  it('skips the interval while hidden, and refreshes when it shows again or the window is focused', () => {
    const refresh = vi.fn();
    const stop = start(refresh);
    doc.visibilityState = 'hidden';
    vi.advanceTimersByTime(DEVICES_REFRESH_MS);
    expect(refresh).not.toHaveBeenCalled();
    doc.visibilityState = 'visible';
    doc.dispatchEvent(new Event('visibilitychange'));
    win.dispatchEvent(new Event('focus'));
    expect(refresh).toHaveBeenCalledTimes(2);
    stop();
  });

  it('stops for good', () => {
    const refresh = vi.fn();
    start(refresh)();
    vi.advanceTimersByTime(DEVICES_REFRESH_MS * 3);
    win.dispatchEvent(new Event('focus'));
    expect(refresh).not.toHaveBeenCalled();
  });
});
