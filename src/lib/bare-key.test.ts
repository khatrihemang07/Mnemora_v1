import { describe, expect, it } from 'vitest';
import { type BareKeyEvent, bareKeyFires, isTypingTarget, overlayOpenIn } from './bare-key';

const press = (over: Partial<BareKeyEvent> = {}): BareKeyEvent => ({
  key: 'q',
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...over,
});

/** An element that matches the selectors of the kinds it is inside. */
const inside = (...tags: string[]) => ({
  closest: (selector: string) =>
    tags.some((t) => selector.split(',').some((s) => s.trim().startsWith(t))) ? {} : null,
});

const free = { target: inside(), overlayOpen: false };

describe('a bare key', () => {
  it('fires on the page, in either case', () => {
    expect(bareKeyFires(press(), 'q', free)).toBe(true);
    expect(bareKeyFires(press({ key: 'Q' }), 'q', free)).toBe(true);
    expect(bareKeyFires(press(), 'Q', free)).toBe(true);
  });

  it('is only its own key', () => {
    expect(bareKeyFires(press({ key: 'w' }), 'q', free)).toBe(false);
    expect(bareKeyFires(press({ key: 'Enter' }), 'q', free)).toBe(false);
  });

  it('never fires with a modifier held', () => {
    for (const mod of ['metaKey', 'ctrlKey', 'altKey', 'shiftKey'] as const) {
      expect(bareKeyFires(press({ [mod]: true }), 'q', free), mod).toBe(false);
    }
  });

  it('never fires while someone is typing', () => {
    for (const field of ['input', 'textarea', 'select', '[contenteditable', '[role="textbox"]']) {
      expect(bareKeyFires(press(), 'q', { target: inside(field), overlayOpen: false }), field).toBe(
        false,
      );
    }
  });

  it('never fires with a dialog or menu open', () => {
    expect(bareKeyFires(press(), 'q', { target: inside(), overlayOpen: true })).toBe(false);
  });

  it('takes Shift only for "?", which is typed with it', () => {
    expect(bareKeyFires(press({ key: '?', shiftKey: true }), '?', free)).toBe(true);
    expect(bareKeyFires(press({ key: '?' }), '?', free)).toBe(true);
    expect(bareKeyFires(press({ key: 'Q', shiftKey: true }), 'q', free)).toBe(false);
  });

  it('ignores held-down repeats, composition and keys already handled', () => {
    expect(bareKeyFires(press({ repeat: true }), 'q', free)).toBe(false);
    expect(bareKeyFires(press({ isComposing: true }), 'q', free)).toBe(false);
    expect(bareKeyFires(press({ defaultPrevented: true }), 'q', free)).toBe(false);
  });

  it('fires when the target is not an element at all', () => {
    expect(bareKeyFires(press(), 'q', { target: null, overlayOpen: false })).toBe(true);
    expect(isTypingTarget({} as EventTarget)).toBe(false);
  });
});

describe('overlayOpenIn', () => {
  it('sees a dialog, alert dialog, menu or listbox', () => {
    for (const role of ['dialog', 'alertdialog', 'menu', 'listbox']) {
      const doc = {
        querySelector: (s: string) => (s.includes(`[role="${role}"]`) ? {} : null),
      };
      expect(overlayOpenIn(doc), role).toBe(true);
    }
    expect(overlayOpenIn({ querySelector: () => null })).toBe(false);
  });
});
