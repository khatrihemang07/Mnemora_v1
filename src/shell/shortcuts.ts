import { shortcutLabel } from '../lib/platform';

export type ShortcutAction = { type: 'palette' } | { type: 'sidebar' };

type KeyLike = Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>;

/**
 * Cmd on macOS, Ctrl elsewhere. Matches the physical key (`code`) as well as the character
 * (`key`). Only the palette and the sidebar have modifier keys; navigation is bare keys (ADR 0011).
 * `\` accepts Alt/Shift, because on some layouts that key is only reachable with them.
 */
export function resolveShortcut(e: KeyLike, mac: boolean): ShortcutAction | null {
  if (mac ? !e.metaKey : !e.ctrlKey) return null;
  if (e.code === 'Backslash' || e.key === '\\') return { type: 'sidebar' };
  if (e.altKey || e.shiftKey) return null;
  if (e.code === 'KeyK' || e.key === 'k') return { type: 'palette' };
  return null;
}

export const SHORTCUT_LIST = [
  { label: 'Command palette', keys: shortcutLabel('K') },
  { label: 'Toggle sidebar', keys: shortcutLabel('\\') },
  { label: 'Go to Home', keys: 'H' },
  { label: 'Go to Journal', keys: 'J' },
  { label: 'Go to Todo', keys: 'T' },
  { label: 'Go to Todo Inbox', keys: 'I' },
  { label: 'Go to Sync', keys: 'S' },
  { label: 'Search the open Journal or Todo', keys: '/' },
  { label: 'Open Keyboard shortcuts in Settings', keys: '?' },
  { label: 'Todo: quick add a Task (anywhere, when not typing)', keys: 'Q' },
  { label: 'Todo: search Tasks', keys: shortcutLabel('F') },
  { label: 'Todo: move the focus ring between Tasks', keys: ['↑', '↓'] },
  { label: 'Todo: complete or reopen the focused Task', keys: 'Space' },
  { label: 'Todo: Won’t do the focused Task (Skip a repeating one)', keys: '⇧Space' },
  { label: 'Todo: open the focused Task', keys: 'Enter' },
  { label: 'Todo: quick edit the focused Task', keys: 'E' },
  { label: 'Todo: pick a date (When) for the focused Task', keys: 'D' },
  { label: 'Todo: set the focused Task due today', keys: 'T' },
  { label: 'Todo: move the focused Task to a List', keys: 'M' },
  { label: 'Todo: set the focused Task’s Priority to 1 to 4', keys: '1–4' },
  { label: 'Todo: reorder the focused Task within its group', keys: ['⌥↑', '⌥↓'] },
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
  { label: 'Journal: finish writing and keep focus', keys: shortcutLabel('↵') },
  { label: 'Journal: save, clear and leave the composer', keys: 'Esc' },
  { label: 'Journal: move between Entries', keys: ['↑', '↓'] },
  { label: 'Journal: edit the focused Entry', keys: 'Enter' },
  { label: 'Journal: delete the focused Entry', keys: '⌫' },
  { label: 'Journal: move the focused Entry', keys: 'M' },
  { label: 'Journal: copy the focused Entry', keys: 'C' },
  { label: 'Journal: from the empty composer to the newest Entry', keys: '↑' },
];
