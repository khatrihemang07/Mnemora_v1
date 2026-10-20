import { describe, expect, it } from 'vitest';
import { resolveTheme } from './resolve';

const media = (dark: boolean) => (q: string) =>
  ({ matches: q.includes('dark') ? dark : false }) as MediaQueryList;

describe('resolveTheme', () => {
  it('system follows the OS', () => {
    expect(resolveTheme('system', media(true))).toBe('dark');
    expect(resolveTheme('system', media(false))).toBe('light');
  });
  it('explicit choices pass through, including oled', () => {
    expect(resolveTheme('light', media(true))).toBe('light');
    expect(resolveTheme('dark', media(false))).toBe('dark');
    expect(resolveTheme('oled', media(false))).toBe('oled');
  });
});
