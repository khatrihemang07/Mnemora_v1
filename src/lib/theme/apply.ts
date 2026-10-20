import type { CoreSettings } from '../settings/core';
import { type ResolvedTheme, resolveTheme } from './resolve';

type Appearance = Pick<CoreSettings, 'theme' | 'density' | 'textSize' | 'reduceMotion'>;

/** Mirrors what Rust's pre-paint script sets, so live changes and first paint agree. */
export function applyAppearance(root: HTMLElement, a: Appearance, resolved?: ResolvedTheme) {
  root.dataset.theme = resolved ?? resolveTheme(a.theme, window.matchMedia.bind(window));
  root.dataset.themePref = a.theme;
  root.dataset.density = a.density;
  root.dataset.textSize = a.textSize;
  root.dataset.reduceMotion = String(a.reduceMotion);
}

/** Re-resolves "system" when the OS flips between light and dark. Returns a stop function. */
export function watchSystemTheme(onChange: () => void): () => void {
  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  mq.addEventListener('change', onChange);
  return () => mq.removeEventListener('change', onChange);
}
