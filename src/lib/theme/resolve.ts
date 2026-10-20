export type ThemePref = 'system' | 'light' | 'dark' | 'oled';
export type ResolvedTheme = 'light' | 'dark' | 'oled';

export const DARK_QUERY = '(prefers-color-scheme: dark)';

/** `matchMedia` is injected so this stays testable without a DOM. */
export function resolveTheme(
  pref: ThemePref,
  matchMedia: (query: string) => MediaQueryList,
): ResolvedTheme {
  if (pref !== 'system') return pref;
  return matchMedia(DARK_QUERY).matches ? 'dark' : 'light';
}
