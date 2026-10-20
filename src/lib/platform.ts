import { isTauri } from '@tauri-apps/api/core';

export type Os = 'macos' | 'windows' | 'linux' | 'android' | 'ios' | 'web';

export function detectOs(ua: string = navigator.userAgent): Os {
  if (/android/i.test(ua)) return 'android';
  if (/iphone|ipad/i.test(ua)) return 'ios';
  if (/mac/i.test(ua)) return 'macos';
  if (/win/i.test(ua)) return 'windows';
  if (/linux|x11/i.test(ua)) return 'linux';
  return 'web';
}

export const os = detectOs();
export const isMac = os === 'macos';
export const isMobileOs = os === 'android' || os === 'ios';
/** True only inside a real Tauri window; false in a browser (including the dev mock). */
export const inTauriWindow = () => isTauri();

/** Cmd on macOS, Ctrl elsewhere. */
export const modLabel = isMac ? '⌘' : 'Ctrl';

/** Display form of a shortcut such as `K` or `\`, e.g. "⌘K" / "Ctrl+K". */
export const shortcutLabel = (key: string) => (isMac ? `${modLabel}${key}` : `${modLabel}+${key}`);

export const hasMod = (e: KeyboardEvent) => (isMac ? e.metaKey : e.ctrlKey) && !e.altKey;
