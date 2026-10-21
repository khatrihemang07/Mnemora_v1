import { describe, expect, it } from 'vitest';
import {
  CHORD_MS,
  IDLE_KEYS,
  type KeyAction,
  type KeyEventLike,
  type KeyScene,
  type KeyState,
  moveRing,
  reduceKey,
  ringAfterRemoval,
} from './task-keymap';

const key = (k: string, mods: Partial<KeyEventLike> = {}): KeyEventLike => ({
  key: k,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});
const scene = (s: Partial<KeyScene> = {}): KeyScene => ({
  typing: false,
  overlay: false,
  sheet: false,
  target: 'ring',
  taskOpen: true,
  onControl: false,
  ...s,
});
const run = (e: KeyEventLike, s: KeyScene = scene(), state: KeyState = IDLE_KEYS, now = 0) =>
  reduceKey(state, e, s, now);
const action = (e: KeyEventLike, s: KeyScene = scene()): KeyAction | null => run(e, s).action;

describe('keys on a focused Task', () => {
  it.each([
    [' ', { type: 'complete' }],
    ['Enter', { type: 'open' }],
    ['e', { type: 'quick-edit' }],
    ['E', { type: 'quick-edit' }],
    ['d', { type: 'when' }],
    ['t', { type: 'today' }],
    ['m', { type: 'move-list' }],
    ['1', { type: 'priority', value: 1 }],
    ['2', { type: 'priority', value: 2 }],
    ['3', { type: 'priority', value: 3 }],
    ['4', { type: 'priority', value: 4 }],
    ['Backspace', { type: 'delete' }],
    ['Delete', { type: 'delete' }],
    ['Escape', { type: 'clear-ring' }],
  ] as [string, KeyAction][])('%s', (k, want) => {
    const r = run(key(k));
    expect(r.action).toEqual(want);
    expect(r.handled).toBe(true);
  });

  it('⇧Space is Won’t do and ⌥↑ ⌥↓ reorder', () => {
    expect(action(key(' ', { shiftKey: true }))).toEqual({ type: 'wont' });
    expect(action(key('ArrowUp', { altKey: true }))).toEqual({ type: 'reorder', dir: -1 });
    expect(action(key('ArrowDown', { altKey: true }))).toEqual({ type: 'reorder', dir: 1 });
  });

  it('↑ and ↓ move the ring', () => {
    expect(action(key('ArrowUp'))).toEqual({ type: 'ring', dir: 'up' });
    expect(action(key('ArrowDown'))).toEqual({ type: 'ring', dir: 'down' });
    expect(action(key('ArrowDown'), scene({ target: 'none' }))).toEqual({
      type: 'ring',
      dir: 'down',
    });
  });

  it('a done Task can be reopened, moved or deleted, but not edited, dated or ranked', () => {
    const done = scene({ taskOpen: false });
    expect(action(key(' '), done)).toEqual({ type: 'complete' });
    expect(action(key('m'), done)).toEqual({ type: 'move-list' });
    expect(action(key('Backspace'), done)).toEqual({ type: 'delete' });
    for (const k of ['e', 'd', 't', '1']) expect(action(key(k), done)).toBeNull();
    expect(action(key(' ', { shiftKey: true }), done)).toBeNull();
    expect(action(key('ArrowUp', { altKey: true }), done)).toBeNull();
  });
});

describe('who the key is for', () => {
  const hover = scene({ target: 'hover' });
  const noTarget = scene({ target: 'none' });

  it('the row under the pointer takes the quick keys but not Space, Enter, delete or Esc', () => {
    for (const k of ['e', 'd', 't', 'm', '3']) expect(action(key(k), hover)).not.toBeNull();
    expect(action(key(' ', { shiftKey: true }), hover)).toEqual({ type: 'wont' });
    expect(action(key('ArrowDown', { altKey: true }), hover)).toEqual({ type: 'reorder', dir: 1 });
    for (const k of [' ', 'Enter', 'Backspace', 'Delete', 'Escape']) {
      expect(action(key(k), hover)).toBeNull();
    }
  });

  it('with no Task, every Task key does nothing and nothing is handled', () => {
    for (const k of [' ', 'Enter', 'e', 'd', 't', 'm', '1', 'Backspace', 'Escape']) {
      const r = run(key(k), noTarget);
      expect(r.action, k).toBeNull();
      expect(r.handled, k).toBe(false);
    }
    expect(action(key('ArrowUp', { altKey: true }), noTarget)).toBeNull();
  });

  it('Space and Enter on a button or link are left to the button', () => {
    const control = scene({ onControl: true });
    expect(action(key(' '), control)).toBeNull();
    expect(action(key('Enter'), control)).toBeNull();
    expect(action(key('e'), control)).toEqual({ type: 'quick-edit' });
  });
});

describe('never while typing or with a menu or Dialog open', () => {
  const blocked = [scene({ typing: true }), scene({ overlay: true })];

  it('ignores every bare key, even the arrows, G and Esc', () => {
    for (const s of blocked) {
      for (const k of [
        ' ',
        'Enter',
        'e',
        'd',
        't',
        'm',
        '2',
        'g',
        'Escape',
        'Backspace',
        'ArrowDown',
      ]) {
        const r = run(key(k), s);
        expect(r.action, k).toBeNull();
        expect(r.handled, k).toBe(false);
      }
      expect(action(key('ArrowUp', { altKey: true }), s)).toBeNull();
    }
  });

  it('typing a G does not start a chord', () => {
    const r = run(key('g'), scene({ typing: true }));
    expect(r.state.goAt).toBeNull();
  });

  it('a modifier or a held-down repeat is not a bare key', () => {
    expect(action(key('e', { metaKey: true }))).toBeNull();
    expect(action(key('e', { ctrlKey: true }))).toBeNull();
    expect(action(key('e', { shiftKey: true }))).toBeNull();
    expect(action(key('e', { repeat: true }))).toBeNull();
    expect(action(key('2', { repeat: true }))).toBeNull();
    expect(action(key('e', { isComposing: true }))).toBeNull();
    expect(action(key('e', { defaultPrevented: true }))).toBeNull();
  });
});

describe('G chords', () => {
  const press = (k: string, state: KeyState, now: number, s: KeyScene = scene()) =>
    reduceKey(state, key(k), s, now);

  it.each([
    ['t', 'today'],
    ['u', 'upcoming'],
    ['i', 'inbox'],
    ['a', 'all'],
    ['d', 'done'],
    ['T', 'today'],
  ] as const)('G then %s goes to %s', (k, to) => {
    const g = press('g', IDLE_KEYS, 1000);
    expect(g.state.goAt).toBe(1000);
    expect(g.action).toBeNull();
    const second = press(k, g.state, 1200);
    expect(second.action).toEqual({ type: 'go', to });
    expect(second.state).toEqual(IDLE_KEYS);
  });

  it('the chord beats the single keys: G then T goes to Today, not "set today"', () => {
    const g = press('g', IDLE_KEYS, 0);
    expect(press('t', g.state, 100).action).toEqual({ type: 'go', to: 'today' });
    expect(press('d', g.state, 100).action).toEqual({ type: 'go', to: 'done' });
  });

  it('works with no Task focused', () => {
    const s = scene({ target: 'none' });
    const g = press('g', IDLE_KEYS, 0, s);
    expect(press('u', g.state, 10, s).action).toEqual({ type: 'go', to: 'upcoming' });
  });

  it('times out, and any other key ends it', () => {
    const g = press('g', IDLE_KEYS, 0);
    expect(press('t', g.state, CHORD_MS + 1).action).toEqual({ type: 'today' });
    const other = press('x', g.state, 10);
    expect(other.state).toEqual(IDLE_KEYS);
    expect(press('t', other.state, 20).action).toEqual({ type: 'today' });
  });

  it('G then G stays a fresh chord and G then a modifier chord does nothing', () => {
    const g = press('g', IDLE_KEYS, 0);
    expect(press('g', g.state, 50).state.goAt).toBe(50);
    expect(reduceKey(g.state, key('t', { metaKey: true }), scene(), 10).action).toBeNull();
  });

  it('does not chord inside a field or while the sheet is open', () => {
    const g = press('g', IDLE_KEYS, 0);
    expect(press('t', g.state, 10, scene({ typing: true })).action).toBeNull();
    expect(press('t', g.state, 10, scene({ sheet: true })).action).toBeNull();
    expect(press('g', IDLE_KEYS, 0, scene({ sheet: true })).state.goAt).toBeNull();
  });
});

describe('search', () => {
  it('⌘F and Ctrl+F open search, even from a field', () => {
    expect(action(key('f', { metaKey: true }))).toEqual({ type: 'search' });
    expect(action(key('F', { ctrlKey: true }), scene({ typing: true }))).toEqual({
      type: 'search',
    });
  });
  it('not with a Dialog open, in the sheet, or with Shift or Alt', () => {
    expect(action(key('f', { metaKey: true }), scene({ overlay: true }))).toBeNull();
    expect(action(key('f', { metaKey: true }), scene({ sheet: true }))).toBeNull();
    expect(action(key('f', { metaKey: true, shiftKey: true }))).toBeNull();
  });
});

describe('in the Task sheet', () => {
  const sheet = scene({ sheet: true });

  it('⌘↑ ⌘↓ step through the Tasks and ⌘↵ closes', () => {
    expect(action(key('ArrowUp', { metaKey: true }), sheet)).toEqual({
      type: 'sheet-step',
      dir: -1,
    });
    expect(action(key('ArrowDown', { ctrlKey: true }), sheet)).toEqual({
      type: 'sheet-step',
      dir: 1,
    });
    expect(action(key('Enter', { metaKey: true }), sheet)).toEqual({ type: 'sheet-close' });
  });

  it('these work from inside the editor too, because the sheet owns them', () => {
    const typing = scene({ sheet: true, typing: true });
    expect(action(key('ArrowDown', { metaKey: true }), typing)).toEqual({
      type: 'sheet-step',
      dir: 1,
    });
    expect(action(key('Enter', { metaKey: true }), typing)).toEqual({ type: 'sheet-close' });
  });

  it('keeps Priority and Won’t do, and nothing else, for the sheet’s Task', () => {
    expect(action(key('2'), sheet)).toEqual({ type: 'priority', value: 2 });
    expect(action(key(' ', { shiftKey: true }), sheet)).toEqual({ type: 'wont' });
    for (const k of [' ', 'Enter', 'e', 'd', 't', 'm', 'Backspace', 'Delete', 'ArrowDown', 'g']) {
      expect(action(key(k), sheet), k).toBeNull();
    }
    expect(action(key('2'), scene({ sheet: true, taskOpen: false }))).toBeNull();
    expect(action(key('2'), scene({ sheet: true, typing: true }))).toBeNull();
  });
});

describe('the focus ring', () => {
  const ids = ['a', 'b', 'c'];

  it('starts at the first Task going down and the last going up', () => {
    expect(moveRing(ids, null, 'down')).toBe('a');
    expect(moveRing(ids, null, 'up')).toBe('c');
  });
  it('moves one Task at a time and holds at the ends', () => {
    expect(moveRing(ids, 'a', 'down')).toBe('b');
    expect(moveRing(ids, 'b', 'up')).toBe('a');
    expect(moveRing(ids, 'a', 'up')).toBe('a');
    expect(moveRing(ids, 'c', 'down')).toBe('c');
  });
  it('starts over when the focused Task is gone, and has nowhere to go with no Tasks', () => {
    expect(moveRing(ids, 'zz', 'down')).toBe('a');
    expect(moveRing([], null, 'down')).toBeNull();
  });
  it('lands on the next Task when one goes away, else the previous', () => {
    expect(ringAfterRemoval(ids, 'a')).toBe('b');
    expect(ringAfterRemoval(ids, 'b')).toBe('c');
    expect(ringAfterRemoval(ids, 'c')).toBe('b');
    expect(ringAfterRemoval(['a'], 'a')).toBeNull();
    expect(ringAfterRemoval(ids, 'zz')).toBeNull();
  });
});
