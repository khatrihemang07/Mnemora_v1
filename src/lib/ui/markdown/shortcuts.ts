import type { BlockKind, InlineMark } from './format';

/** What an editor shortcut does. */
export type EditorCommand =
  | { type: 'inline'; mark: InlineMark }
  | { type: 'block'; kind: BlockKind }
  | { type: 'link' };

type KeyLike = Pick<KeyboardEvent, 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>;

const INLINE: Record<string, InlineMark> = {
  KeyB: '**',
  KeyI: '*',
  KeyE: '`',
};
const SHIFT_INLINE: Record<string, InlineMark> = {
  KeyX: '~~',
  KeyH: '==',
};
const SHIFT_BLOCK: Record<string, BlockKind> = {
  Digit8: 'bullet',
  Digit7: 'numbered',
  Digit9: 'task',
};
const ALT_BLOCK: Record<string, BlockKind> = {
  Digit1: 'h1',
  Digit2: 'h2',
  Digit3: 'h3',
};

/**
 * Maps a key press to an editor command: ⌘B/⌘I bold/italic, ⌘E code, ⌘⇧X strikethrough, ⌘⇧H
 * highlight, ⌘K link, ⌘⇧8/7/9 bullet/numbered/checklist, ⌘⌥1-3 headings (⌘ is Ctrl off macOS).
 * Uses the physical key (`code`) because Shift and Alt change the typed character. Plain ⌘1-9 is
 * left alone: the shell uses it to switch Modules.
 */
export function resolveEditorShortcut(e: KeyLike, mac: boolean): EditorCommand | null {
  if (mac ? !e.metaKey || e.ctrlKey : !e.ctrlKey || e.metaKey) return null;
  if (e.altKey) {
    const kind = e.shiftKey ? undefined : ALT_BLOCK[e.code];
    return kind ? { type: 'block', kind } : null;
  }
  if (e.shiftKey) {
    const mark = SHIFT_INLINE[e.code];
    if (mark) return { type: 'inline', mark };
    const kind = SHIFT_BLOCK[e.code];
    return kind ? { type: 'block', kind } : null;
  }
  const mark = INLINE[e.code];
  if (mark) return { type: 'inline', mark };
  return e.code === 'KeyK' ? { type: 'link' } : null;
}

/** ⌘⇧V (Ctrl+Shift+V off macOS): the next paste is plain text. Physical key, as Shift changes `key`. */
export function isPlainPasteShortcut(e: KeyLike, mac: boolean): boolean {
  if (mac ? !e.metaKey || e.ctrlKey : !e.ctrlKey || e.metaKey) return false;
  return e.shiftKey && !e.altKey && e.code === 'KeyV';
}
