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
  { label: 'New', keys: shortcutLabel('N') },
  { label: 'Go to Module 1 to 9', keys: shortcutLabel('1–9') },
  { label: 'Todo: quick add a Task (anywhere, when not typing)', keys: 'Q' },
  { label: 'Todo: search Tasks', keys: shortcutLabel('F') },
  { label: 'Todo: move the focus ring between Tasks', keys: ['↑', '↓'] },
  { label: 'Todo: complete or reopen the focused Task', keys: 'Space' },
  { label: 'Todo: Won\u2019t do the Task (Skip a repeating one)', keys: '⇧Space' },
  { label: 'Todo: open the focused Task', keys: 'Enter' },
  { label: 'Todo: quick edit the focused or hovered Task', keys: 'E' },
  { label: 'Todo: pick a date (When) for the Task', keys: 'D' },
  { label: 'Todo: set the Task due today', keys: 'T' },
  { label: 'Todo: move the Task to a List', keys: 'M' },
  { label: 'Todo: set the Priority to 1 to 4', keys: '1–4' },
  { label: 'Todo: reorder the Task within its group', keys: ['⌥↑', '⌥↓'] },
  { label: 'Todo: delete the focused Task (Undo)', keys: ['⌫', 'Delete'] },
  { label: 'Todo: clear the focus ring', keys: 'Esc' },
  {
    label: 'Todo: in a Task sheet, show the previous or next Task',
    keys: [shortcutLabel('↑'), shortcutLabel('↓')],
  },
  { label: 'Todo: in a Task sheet, close it', keys: [shortcutLabel('↵'), 'Esc'] },
  {
    label: 'Todo: G, then T U I A or D goes to Today, Upcoming, Inbox, All or Done',
    keys: ['G', 'T U I A D'],
  },
  { label: 'Journal: start an Entry (anywhere, when not typing)', keys: 'N' },
  { label: 'Journal: search', keys: shortcutLabel('F') },
  { label: 'Journal: go to date', keys: shortcutLabel('G') },
  { label: 'Journal: finish writing (either key)', keys: [shortcutLabel('↵'), 'Esc'] },
  { label: 'Journal: move between Entries', keys: ['↑', '↓'] },
  { label: 'Journal: edit the focused Entry', keys: 'Enter' },
  { label: 'Journal: delete the focused Entry', keys: '⌫' },
  { label: 'Journal: move the focused Entry', keys: 'M' },
  { label: 'Journal: copy the focused Entry', keys: 'C' },
  { label: 'Journal: from the empty composer to the newest Entry', keys: '↑' },
];
