import { describe, expect, it } from 'vitest';
import { isPlainPasteShortcut, resolveEditorShortcut } from './shortcuts';

const key = (
  code: string,
  mods: Partial<Record<'meta' | 'ctrl' | 'alt' | 'shift', boolean>> = {},
) => ({
  code,
  metaKey: !!mods.meta,
  ctrlKey: !!mods.ctrl,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift,
});

describe('resolveEditorShortcut', () => {
  it('maps the inline shortcuts', () => {
    expect(resolveEditorShortcut(key('KeyB', { meta: true }), true)).toEqual({
      type: 'inline',
      mark: '**',
    });
    expect(resolveEditorShortcut(key('KeyI', { meta: true }), true)).toEqual({
      type: 'inline',
      mark: '*',
    });
    expect(resolveEditorShortcut(key('KeyE', { meta: true }), true)).toEqual({
      type: 'inline',
      mark: '`',
    });
    expect(resolveEditorShortcut(key('KeyX', { meta: true, shift: true }), true)).toEqual({
      type: 'inline',
      mark: '~~',
    });
    expect(resolveEditorShortcut(key('KeyH', { meta: true, shift: true }), true)).toEqual({
      type: 'inline',
      mark: '==',
    });
  });

  it('maps lists with Shift and headings with Alt, by physical key', () => {
    expect(resolveEditorShortcut(key('Digit8', { meta: true, shift: true }), true)).toEqual({
      type: 'block',
      kind: 'bullet',
    });
    expect(resolveEditorShortcut(key('Digit7', { meta: true, shift: true }), true)).toEqual({
      type: 'block',
      kind: 'numbered',
    });
    expect(resolveEditorShortcut(key('Digit9', { meta: true, shift: true }), true)).toEqual({
      type: 'block',
      kind: 'task',
    });
    expect(resolveEditorShortcut(key('Digit2', { meta: true, alt: true }), true)).toEqual({
      type: 'block',
      kind: 'h2',
    });
  });

  it('link comes through as a command (the editor decides about the selection)', () => {
    expect(resolveEditorShortcut(key('KeyK', { meta: true }), true)).toEqual({ type: 'link' });
  });

  it('leaves the shell shortcuts alone', () => {
    expect(resolveEditorShortcut(key('Digit1', { meta: true }), true)).toBeNull();
    expect(resolveEditorShortcut(key('Digit4', { meta: true, alt: true }), true)).toBeNull();
    expect(resolveEditorShortcut(key('KeyN', { meta: true }), true)).toBeNull();
    expect(resolveEditorShortcut(key('KeyB'), true)).toBeNull();
  });

  it('uses Ctrl off macOS and never both', () => {
    expect(resolveEditorShortcut(key('KeyB', { ctrl: true }), false)).toEqual({
      type: 'inline',
      mark: '**',
    });
    expect(resolveEditorShortcut(key('KeyB', { meta: true }), false)).toBeNull();
    expect(resolveEditorShortcut(key('KeyB', { ctrl: true }), true)).toBeNull();
    expect(resolveEditorShortcut(key('KeyB', { meta: true, ctrl: true }), true)).toBeNull();
  });
});

describe('isPlainPasteShortcut', () => {
  it('is ⌘⇧V on macOS, by physical key (Shift changes the typed character)', () => {
    expect(isPlainPasteShortcut(key('KeyV', { meta: true, shift: true }), true)).toBe(true);
    expect(isPlainPasteShortcut(key('KeyV', { meta: true }), true)).toBe(false);
    expect(isPlainPasteShortcut(key('KeyV', { ctrl: true, shift: true }), true)).toBe(false);
  });
  it('is Ctrl+Shift+V elsewhere', () => {
    expect(isPlainPasteShortcut(key('KeyV', { ctrl: true, shift: true }), false)).toBe(true);
    expect(isPlainPasteShortcut(key('KeyV', { meta: true, shift: true }), false)).toBe(false);
  });
  it('needs the V key', () => {
    expect(isPlainPasteShortcut(key('KeyC', { meta: true, shift: true }), true)).toBe(false);
  });
});
