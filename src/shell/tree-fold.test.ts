import { describe, expect, it } from 'vitest';
import { initialOpen, onNavigate, rowClick, type TreeFold, toggle } from './tree-fold';

const fold = (open: boolean, inside: boolean): TreeFold => ({ open, inside });

describe('onNavigate', () => {
  it('unfolds on entering and folds on leaving', () => {
    expect(onNavigate(fold(false, false), true)).toEqual(fold(true, true));
    expect(onNavigate(fold(true, true), false)).toEqual(fold(false, false));
  });

  it('keeps the user choice while moving around inside or outside', () => {
    expect(onNavigate(fold(false, true), true)).toEqual(fold(false, true));
    expect(onNavigate(fold(true, false), false)).toEqual(fold(true, false));
  });

  it('applies the automatic rule again after an override', () => {
    let f = toggle(fold(true, true)); // folded by hand while inside
    expect(f.open).toBe(false);
    f = onNavigate(f, false); // leave: stays folded
    expect(f).toEqual(fold(false, false));
    f = onNavigate(f, true); // enter: unfolds again
    expect(f).toEqual(fold(true, true));
    f = onNavigate(toggle(f), false); // folded by hand, then leave
    expect(f.open).toBe(false);
  });

  it('refolds on leaving even when it was unfolded by hand outside', () => {
    const peeked = toggle(fold(false, false));
    expect(peeked.open).toBe(true);
    expect(onNavigate(peeked, true)).toEqual(fold(true, true));
  });
});

describe('toggle', () => {
  it('flips open and nothing else', () => {
    expect(toggle(fold(true, true))).toEqual(fold(false, true));
    expect(toggle(fold(false, false))).toEqual(fold(true, false));
  });
});

describe('rowClick', () => {
  it('flips the tree inside the Module and goes there from outside', () => {
    expect(rowClick(fold(true, true))).toBe('toggle');
    expect(rowClick(fold(false, true))).toBe('toggle');
    expect(rowClick(fold(false, false))).toBe('enter');
    expect(rowClick(fold(true, false))).toBe('enter');
  });
});

describe('initialOpen', () => {
  it('opens inside the Module when nothing was saved, and folds outside it', () => {
    expect(initialOpen(null, true)).toBe(true);
    expect(initialOpen(null, false)).toBe(false);
  });

  it('lets a saved choice win either way', () => {
    expect(initialOpen(false, true)).toBe(false);
    expect(initialOpen(true, false)).toBe(true);
  });
});
