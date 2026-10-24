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
    expect(resolveShortcut(key('k', { ctrlKey: true }), false)).toEqual({ type: 'palette' });
    expect(resolveShortcut(key('k', { metaKey: true }), false)).toBeNull();
  });
  it('maps the palette and the sidebar', () => {
    expect(resolveShortcut(key('\\', { metaKey: true }), true)).toEqual({ type: 'sidebar' });
  });
  it('no longer maps the old navigation and create keys (they are bare keys now)', () => {
    expect(resolveShortcut(key('n', { metaKey: true }), true)).toBeNull();
    expect(resolveShortcut(key('3', { metaKey: true }), true)).toBeNull();
    expect(resolveShortcut(key(',', { metaKey: true }), true)).toBeNull();
  });
  it('ignores plain keys and Alt/Shift combos on letters', () => {
    expect(resolveShortcut(key('k'), true)).toBeNull();
    expect(resolveShortcut(key('k', { metaKey: true, shiftKey: true }, 'KeyK'), true)).toBeNull();
  });
});

describe('resolveShortcut by physical key', () => {
  it('matches e.code when the produced character differs', () => {
    const meta = { metaKey: true };
    expect(resolveShortcut(key('«', meta, 'Backslash'), true)).toEqual({ type: 'sidebar' });
    expect(resolveShortcut(key('ç', meta, 'KeyK'), true)).toEqual({ type: 'palette' });
    expect(resolveShortcut(key('ñ', meta, 'KeyN'), true)).toBeNull();
  });
  it('lets Backslash through with Shift or Alt (layouts that need them)', () => {
    expect(resolveShortcut(key('|', { metaKey: true, altKey: true }, 'Backslash'), true)).toEqual({
      type: 'sidebar',
    });
  });
  it('rejects unrelated codes', () => {
    expect(resolveShortcut(key('x', { metaKey: true }, 'KeyX'), true)).toBeNull();
  });
});
