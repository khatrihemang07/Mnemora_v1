/**
 * Bare single-key shortcuts that work anywhere in the app (Q for Todo's quick add, later N for
 * the Journal; ADR 0011). They must never steal a letter someone is typing, so a bare key fires
 * only when nothing is being typed into, no modifier is held and no dialog or menu is open.
 */

/** The parts of a key event the rule reads. */
export interface BareKeyEvent {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  repeat?: boolean;
  isComposing?: boolean;
  defaultPrevented?: boolean;
}

/** Just enough of an element to ask whether it takes text. */
export interface TypingTarget {
  closest: (selector: string) => unknown;
}

/** Elements a letter key types into. */
const TEXT_FIELDS =
  'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"]';

/** Whether a key aimed at `target` is going into a text field. */
export function isTypingTarget(target: EventTarget | TypingTarget | null): boolean {
  const el = target as Partial<TypingTarget> | null;
  return typeof el?.closest === 'function' && el.closest(TEXT_FIELDS) !== null;
}

/** What is on screen, as far as a bare key cares. */
export interface BareKeyScene {
  /** Where the key was aimed. */
  target: EventTarget | TypingTarget | null;
  /** A dialog, sheet, menu or picker is open. */
  overlayOpen: boolean;
}

/** Whether `e` is the bare key `key` (a letter, either case) and may fire in `scene`. */
export function bareKeyFires(e: BareKeyEvent, key: string, scene: BareKeyScene): boolean {
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  // Shift is how "?" is typed, so only that key takes it.
  if (e.shiftKey && key !== '?') return false;
  if (e.repeat || e.isComposing || e.defaultPrevented) return false;
  if (e.key.toLowerCase() !== key.toLowerCase()) return false;
  return !scene.overlayOpen && !isTypingTarget(scene.target);
}

/** Whether anything modal is open in `doc`: a Dialog, an alert Dialog, a menu or a listbox. */
export const overlayOpenIn = (doc: Pick<Document, 'querySelector'>): boolean =>
  doc.querySelector('[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]') !==
  null;
