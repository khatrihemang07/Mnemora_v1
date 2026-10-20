import { shortcutLabel } from '../lib/platform';

export type ShortcutAction =
  | { type: 'palette' }
  | { type: 'settings' }
  | { type: 'sidebar' }
  | { type: 'create' }
  | { type: 'module'; digit: number };

type KeyLike = Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>;

/**
 * Cmd on macOS, Ctrl elsewhere. Matches the physical key (`code`) as well as the character
 * (`key`). Letters and digits ignore Alt/Shift combos; `\` and `,` accept them, because on
 * some layouts those keys are only reachable with Shift or Alt.
 */
export function resolveShortcut(e: KeyLike, mac: boolean): ShortcutAction | null {
  if (mac ? !e.metaKey : !e.ctrlKey) return null;
  if (e.code === 'Comma' || e.key === ',') return { type: 'settings' };
  if (e.code === 'Backslash' || e.key === '\\') return { type: 'sidebar' };
  if (e.altKey || e.shiftKey) return null;
  if (e.code === 'KeyK' || e.key === 'k') return { type: 'palette' };
  if (e.code === 'KeyN' || e.key === 'n') return { type: 'create' };
  const digit = /^Digit([1-9])$/.exec(e.code)?.[1] ?? (/^[1-9]$/.test(e.key) ? e.key : null);
  if (digit) return { type: 'module', digit: Number(digit) };
  return null;
}

export const SHORTCUT_LIST = [
  { label: 'Command palette', keys: shortcutLabel('K') },
  { label: 'Open Settings', keys: shortcutLabel(',') },
  { label: 'Toggle sidebar', keys: shortcutLabel('\\') },
  { label: 'New (not available yet)', keys: shortcutLabel('N') },
  { label: 'Go to Module 1 to 9', keys: shortcutLabel('1–9') },
];
