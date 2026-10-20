import { describe, expect, it } from 'vitest';
import { focusOpenAllowed, shownShortcut, tooltipAllowed } from './tooltip-gate';

describe('tooltip gate', () => {
  it('blocks touch and pen, allows mouse, keyboard and unknown', () => {
    expect(tooltipAllowed('touch')).toBe(false);
    expect(tooltipAllowed('pen')).toBe(false);
    expect(tooltipAllowed('mouse')).toBe(true);
    expect(tooltipAllowed(null)).toBe(true);
  });

  it('opens on focus only for keyboard focus-visible', () => {
    expect(focusOpenAllowed(true, true)).toBe(true);
    expect(focusOpenAllowed(true, false)).toBe(false);
    expect(focusOpenAllowed(false, true)).toBe(false);
    expect(focusOpenAllowed(false, false)).toBe(false);
  });

  it('hides the shortcut on a phone only', () => {
    expect(shownShortcut('Ctrl+G', true)).toBeUndefined();
    expect(shownShortcut('Ctrl+G', false)).toBe('Ctrl+G');
    expect(shownShortcut(undefined, false)).toBeUndefined();
  });
});
