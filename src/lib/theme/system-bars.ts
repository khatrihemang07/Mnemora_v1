import type { ResolvedTheme } from './resolve';

/** Does what the Android MainActivity exposes to the WebView; absent everywhere else. */
export interface AndroidBridge {
  setSystemBars?: (dark: boolean, colorHex: string) => void;
}

const HEX_DIGITS = 2;
const toHexByte = (n: number) => n.toString(16).padStart(HEX_DIGITS, '0');

/** Icons are light-on-dark for every theme except light. */
export const isDarkTheme = (theme: ResolvedTheme): boolean => theme !== 'light';

/** Converts any CSS colour (including oklch) to `#rrggbb` by painting one canvas pixel. */
export function cssColorToHex(css: string): string | null {
  const ctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;
  ctx.fillStyle = css;
  ctx.fillRect(0, 0, 1, 1);
  const pixel = ctx.getImageData(0, 0, 1, 1).data;
  return `#${Array.from(pixel.slice(0, 3), toHexByte).join('')}`;
}

/** Tells the native shell to restyle the status and navigation bars. No-op without the bridge. */
export function syncSystemBars(
  bridge: AndroidBridge | undefined,
  theme: ResolvedTheme,
  readBackground: () => string | null,
): boolean {
  if (typeof bridge?.setSystemBars !== 'function') return false;
  const color = readBackground();
  if (!color) return false;
  bridge.setSystemBars(isDarkTheme(theme), color);
  return true;
}

/** Browser-side wiring: reads the body background after the theme attributes were applied. */
export function applySystemBars(theme: ResolvedTheme): void {
  syncSystemBars(window.MnemoraAndroid, theme, () =>
    cssColorToHex(getComputedStyle(document.body).backgroundColor),
  );
}
