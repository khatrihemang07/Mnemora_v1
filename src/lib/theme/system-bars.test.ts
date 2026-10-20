import { describe, expect, it, vi } from 'vitest';
import { isDarkTheme, syncSystemBars } from './system-bars';

describe('syncSystemBars', () => {
  it('does nothing without the Android bridge', () => {
    expect(syncSystemBars(undefined, 'dark', () => '#000000')).toBe(false);
    expect(syncSystemBars({}, 'dark', () => '#000000')).toBe(false);
  });
  it('passes dark=false only for the light theme', () => {
    const setSystemBars = vi.fn();
    syncSystemBars({ setSystemBars }, 'light', () => '#ffffff');
    syncSystemBars({ setSystemBars }, 'oled', () => '#000000');
    expect(setSystemBars).toHaveBeenNthCalledWith(1, false, '#ffffff');
    expect(setSystemBars).toHaveBeenNthCalledWith(2, true, '#000000');
  });
  it('skips the call when the colour cannot be read', () => {
    const setSystemBars = vi.fn();
    expect(syncSystemBars({ setSystemBars }, 'dark', () => null)).toBe(false);
    expect(setSystemBars).not.toHaveBeenCalled();
  });
  it('treats dark and oled as dark', () => {
    expect(isDarkTheme('dark')).toBe(true);
    expect(isDarkTheme('oled')).toBe(true);
    expect(isDarkTheme('light')).toBe(false);
  });
});
