import { describe, expect, it } from 'vitest';
import { shownShortcut, tooltipAllowed } from './tooltip-gate';

describe('tooltip gate', () => {
  it('blocks touch and pen, allows mouse, keyboard and unknown', () => {
    expect(tooltipAllowed('touch')).toBe(false);
    expect(tooltipAllowed('pen')).toBe(false);
    expect(tooltipAllowed('mouse')).toBe(true);
    expect(tooltipAllowed(null)).toBe(true);
  });

  it('hides the shortcut on a phone only', () => {
    expect(shownShortcut('Ctrl+G', true)).toBeUndefined();
    expect(shownShortcut('Ctrl+G', false)).toBe('Ctrl+G');
    expect(shownShortcut(undefined, false)).toBeUndefined();
  });
});
