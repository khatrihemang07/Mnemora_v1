import { describe, expect, it } from 'vitest';
import { draggedWidth, paneLimits, percentToPx, pxToPercent } from './pane-math';

describe('pixel/percent conversion', () => {
  it('converts both ways', () => {
    expect(pxToPercent(250, 1000)).toBe(25);
    expect(percentToPx(25, 1000)).toBe(250);
  });
  it('is 0% when the group has no width yet', () => {
    expect(pxToPercent(250, 0)).toBe(0);
  });
  it('limits run from min to max', () => {
    expect(paneLimits(60, 360, 1200)).toEqual({ min: 5, max: 30 });
  });
});

describe('draggedWidth', () => {
  it('ignores echoes of the width we set', () => {
    expect(draggedWidth(24.9, 1000, 248)).toBeNull();
  });
  it('reports real drags in px', () => {
    expect(draggedWidth(30, 1000, 248)).toBe(300);
  });
});
