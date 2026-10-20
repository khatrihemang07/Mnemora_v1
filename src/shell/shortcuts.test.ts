import { describe, expect, it } from 'vitest';
import { resolveShortcut } from './shortcuts';

type Mods = Partial<Record<'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey', boolean>>;
const key = (k: string, mods: Mods = {}, code = '') => ({
  key: k,
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe('resolveShortcut', () => {
  it('uses Meta on macOS', () => {
    expect(resolveShortcut(key('k', { metaKey: true }), true)).toEqual({ type: 'palette' });
    expect(resolveShortcut(key('k', { ctrlKey: true }), true)).toBeNull();
  });
  it('uses Ctrl elsewhere', () => {
    expect(resolveShortcut(key(',', { ctrlKey: true }), false)).toEqual({ type: 'settings' });
    expect(resolveShortcut(key(',', { metaKey: true }), false)).toBeNull();
  });
  it('maps sidebar, create and digits', () => {
    expect(resolveShortcut(key('\\', { metaKey: true }), true)).toEqual({ type: 'sidebar' });
    expect(resolveShortcut(key('n', { metaKey: true }), true)).toEqual({ type: 'create' });
    expect(resolveShortcut(key('3', { metaKey: true }), true)).toEqual({
      type: 'module',
      digit: 3,
    });
    expect(resolveShortcut(key('0', { metaKey: true }), true)).toBeNull();
  });
  it('ignores plain keys and Alt/Shift combos on letters and digits', () => {
    expect(resolveShortcut(key('k'), true)).toBeNull();
    expect(resolveShortcut(key('k', { metaKey: true, shiftKey: true }, 'KeyK'), true)).toBeNull();
    expect(resolveShortcut(key('1', { metaKey: true, altKey: true }, 'Digit1'), true)).toBeNull();
  });
});

describe('resolveShortcut by physical key', () => {
  it('matches e.code when the produced character differs', () => {
    const meta = { metaKey: true };
    expect(resolveShortcut(key('«', meta, 'Backslash'), true)).toEqual({ type: 'sidebar' });
    expect(resolveShortcut(key('ç', meta, 'KeyK'), true)).toEqual({ type: 'palette' });
    expect(resolveShortcut(key('ñ', meta, 'KeyN'), true)).toEqual({ type: 'create' });
    expect(resolveShortcut(key('&', meta, 'Digit1'), true)).toEqual({ type: 'module', digit: 1 });
    expect(resolveShortcut(key('(', meta, 'Digit9'), true)).toEqual({ type: 'module', digit: 9 });
  });
  it('lets Backslash and Comma through with Shift or Alt (layouts that need them)', () => {
    expect(resolveShortcut(key('|', { metaKey: true, altKey: true }, 'Backslash'), true)).toEqual({
      type: 'sidebar',
    });
    expect(resolveShortcut(key(';', { ctrlKey: true, shiftKey: true }, 'Comma'), false)).toEqual({
      type: 'settings',
    });
  });
  it('rejects Digit0 and unrelated codes', () => {
    expect(resolveShortcut(key('0', { metaKey: true }, 'Digit0'), true)).toBeNull();
    expect(resolveShortcut(key('x', { metaKey: true }, 'KeyX'), true)).toBeNull();
  });
});
